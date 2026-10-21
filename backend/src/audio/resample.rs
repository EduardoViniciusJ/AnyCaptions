use audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Indexing, Resampler};

pub const OUTPUT_RATE: u32 = 16_000;
const CHUNK_FRAMES: usize = 1024;
const MAX_INPUT_FRAMES: usize = 65_536;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResampleError {
    UnsupportedRate,
    InvalidInput,
    ProcessingFailed,
}

pub struct MonoResampler {
    input_rate: u32,
    engine: Option<Fft<f32>>,
    input: Vec<f32>,
    filled: usize,
    output: Vec<f32>,
    delay: usize,
}

impl MonoResampler {
    pub fn new(input_rate: u32) -> Result<Self, ResampleError> {
        if !(8_000..=192_000).contains(&input_rate) {
            return Err(ResampleError::UnsupportedRate);
        }
        let engine = if input_rate == OUTPUT_RATE {
            None
        } else {
            Some(
                Fft::<f32>::new(
                    input_rate as usize,
                    OUTPUT_RATE as usize,
                    CHUNK_FRAMES,
                    1,
                    FixedSync::Input,
                )
                .map_err(|_| ResampleError::UnsupportedRate)?,
            )
        };
        let input_len = engine
            .as_ref()
            .map_or(CHUNK_FRAMES, Resampler::input_frames_max);
        let output_len = engine
            .as_ref()
            .map_or(CHUNK_FRAMES, Resampler::output_frames_max);
        let delay = engine.as_ref().map_or(0, Resampler::output_delay);
        Ok(Self {
            input_rate,
            engine,
            input: vec![0.0; input_len],
            filled: 0,
            output: vec![0.0; output_len],
            delay,
        })
    }

    pub fn input_rate(&self) -> u32 {
        self.input_rate
    }
    pub fn output_delay(&self) -> usize {
        self.delay
    }
    pub fn buffer_capacity_frames(&self) -> usize {
        self.input.len() + self.output.len()
    }

    pub fn reset_for_rate(&mut self, input_rate: u32) -> Result<(), ResampleError> {
        if input_rate != self.input_rate {
            *self = Self::new(input_rate)?;
        } else {
            self.reset();
        }
        Ok(())
    }

    pub fn reset(&mut self) {
        self.filled = 0;
        if let Some(engine) = &mut self.engine {
            engine.reset();
        }
    }

    pub fn push(&mut self, samples: &[f32], out: &mut Vec<f32>) -> Result<(), ResampleError> {
        if samples.len() > MAX_INPUT_FRAMES || samples.iter().any(|sample| !sample.is_finite()) {
            return Err(ResampleError::InvalidInput);
        }
        if self.engine.is_none() {
            out.extend_from_slice(samples);
            return Ok(());
        }
        let mut remaining = samples;
        while !remaining.is_empty() {
            let needed = self
                .engine
                .as_ref()
                .ok_or(ResampleError::ProcessingFailed)?
                .input_frames_next();
            let count = (needed - self.filled).min(remaining.len());
            self.input[self.filled..self.filled + count].copy_from_slice(&remaining[..count]);
            self.filled += count;
            remaining = &remaining[count..];
            if self.filled == needed {
                self.process(None, out)?;
                self.filled = 0;
            }
        }
        Ok(())
    }

    pub fn finish(&mut self, out: &mut Vec<f32>) -> Result<(), ResampleError> {
        if self.filled > 0 {
            let partial_len = self.filled;
            self.process(Some(partial_len), out)?;
        }
        self.reset();
        Ok(())
    }

    fn process(
        &mut self,
        partial_len: Option<usize>,
        out: &mut Vec<f32>,
    ) -> Result<(), ResampleError> {
        let engine = self
            .engine
            .as_mut()
            .ok_or(ResampleError::ProcessingFailed)?;
        let input = InterleavedSlice::new(self.input.as_slice(), 1, self.input.len())
            .map_err(|_| ResampleError::ProcessingFailed)?;
        let output_len = self.output.len();
        let mut output = InterleavedSlice::new_mut(self.output.as_mut_slice(), 1, output_len)
            .map_err(|_| ResampleError::ProcessingFailed)?;
        let indexing = partial_len.map(|len| Indexing {
            partial_len: Some(len),
            ..Default::default()
        });
        let (_, produced) = engine
            .process_into_buffer(&input, &mut output, indexing.as_ref())
            .map_err(|_| ResampleError::ProcessingFailed)?;
        out.extend_from_slice(&self.output[..produced]);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(rate: u32, seconds: usize) -> Vec<f32> {
        (0..rate as usize * seconds)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / rate as f32).sin())
            .collect()
    }

    #[test]
    fn tone_frequency_and_duration_are_preserved_at_common_rates() {
        for rate in [32_000, 44_100, 48_000] {
            let mut resampler = MonoResampler::new(rate).unwrap();
            let delay = resampler.output_delay();
            let mut out = Vec::new();
            for chunk in sine(rate, 1).chunks(317) {
                resampler.push(chunk, &mut out).unwrap();
            }
            resampler.finish(&mut out).unwrap();
            assert!(
                (out.len() as isize - OUTPUT_RATE as isize).abs() < 500,
                "rate {rate}, length {}",
                out.len()
            );
            let steady = &out[delay + 500..delay + 15_000];
            let crossings = steady
                .windows(2)
                .filter(|pair| pair[0] <= 0.0 && pair[1] > 0.0)
                .count();
            let frequency = crossings as f64 * OUTPUT_RATE as f64 / steady.len() as f64;
            assert!(
                (frequency - 440.0).abs() < 2.0,
                "rate {rate}, frequency {frequency}"
            );
            assert!(out.iter().all(|sample| sample.is_finite()));
            assert!(resampler.buffer_capacity_frames() < 10_000);
        }
    }

    #[test]
    fn block_boundaries_do_not_reset_filter_state() {
        let input = sine(48_000, 1);
        let mut a = MonoResampler::new(48_000).unwrap();
        let mut b = MonoResampler::new(48_000).unwrap();
        let (mut whole, mut split) = (Vec::new(), Vec::new());
        for chunk in input.chunks(MAX_INPUT_FRAMES) {
            a.push(chunk, &mut whole).unwrap();
        }
        for chunk in input.chunks(173) {
            b.push(chunk, &mut split).unwrap();
        }
        a.finish(&mut whole).unwrap();
        b.finish(&mut split).unwrap();
        assert_eq!(whole, split);
    }

    #[test]
    fn silence_remains_silent_and_rate_change_resets() {
        let mut resampler = MonoResampler::new(48_000).unwrap();
        let mut out = Vec::new();
        resampler.push(&vec![0.0; 48_000], &mut out).unwrap();
        resampler.finish(&mut out).unwrap();
        assert!(out.iter().all(|value| value.abs() < 0.000001));
        resampler.reset_for_rate(44_100).unwrap();
        assert_eq!(resampler.input_rate(), 44_100);
        assert_eq!(
            resampler.push(&[f32::NAN], &mut out),
            Err(ResampleError::InvalidInput)
        );
    }
}
