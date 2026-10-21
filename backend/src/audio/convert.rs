use wasapi::{SampleType, WaveFormat};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcmEncoding {
    Float32,
    Int16,
    Int24,
    Int24In32,
    Int32,
}

impl PcmEncoding {
    fn bytes(self) -> usize {
        match self {
            Self::Int16 => 2,
            Self::Int24 => 3,
            Self::Float32 | Self::Int24In32 | Self::Int32 => 4,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PcmFormat {
    pub sample_rate: u32,
    pub channels: u16,
    pub encoding: PcmEncoding,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConvertError {
    UnsupportedFormat,
    InvalidFrame,
    NonFiniteSample,
}

impl PcmFormat {
    pub fn from_wave_format(wave: &WaveFormat) -> Result<Self, ConvertError> {
        let channels = wave.get_nchannels();
        let sample_rate = wave.get_samplespersec();
        if channels == 0 || sample_rate == 0 {
            return Err(ConvertError::UnsupportedFormat);
        }
        let encoding = match (
            wave.get_subformat(),
            wave.get_bitspersample(),
            wave.get_validbitspersample(),
        ) {
            (Ok(SampleType::Float), 32, 32) => PcmEncoding::Float32,
            (Ok(SampleType::Int), 16, 16) => PcmEncoding::Int16,
            (Ok(SampleType::Int), 24, 24) => PcmEncoding::Int24,
            (Ok(SampleType::Int), 32, 24) => PcmEncoding::Int24In32,
            (Ok(SampleType::Int), 32, 32) => PcmEncoding::Int32,
            _ => return Err(ConvertError::UnsupportedFormat),
        };
        let format = Self {
            sample_rate,
            channels,
            encoding,
        };
        if wave.get_blockalign() as usize != format.frame_bytes() {
            return Err(ConvertError::UnsupportedFormat);
        }
        Ok(format)
    }

    pub fn frame_bytes(self) -> usize {
        self.channels as usize * self.encoding.bytes()
    }
}

/// Equal average of all channels; opposite-polarity channels can cancel each other.
pub fn to_mono_f32(
    bytes: &[u8],
    format: PcmFormat,
    silent: bool,
    out: &mut Vec<f32>,
) -> Result<(), ConvertError> {
    let frame_bytes = format.frame_bytes();
    if frame_bytes == 0 || !bytes.len().is_multiple_of(frame_bytes) {
        return Err(ConvertError::InvalidFrame);
    }
    out.clear();
    out.reserve(bytes.len() / frame_bytes);
    for frame in bytes.chunks_exact(frame_bytes) {
        if silent {
            out.push(0.0);
            continue;
        }
        let mut sum = 0.0_f64;
        for sample in frame.chunks_exact(format.encoding.bytes()) {
            let value = match format.encoding {
                PcmEncoding::Float32 => {
                    f32::from_le_bytes(sample.try_into().map_err(|_| ConvertError::InvalidFrame)?)
                        as f64
                }
                PcmEncoding::Int16 => {
                    i16::from_le_bytes(sample.try_into().map_err(|_| ConvertError::InvalidFrame)?)
                        as f64
                        / 32768.0
                }
                PcmEncoding::Int24 => {
                    let raw = i32::from_le_bytes([
                        sample[0],
                        sample[1],
                        sample[2],
                        if sample[2] & 0x80 == 0 { 0 } else { 0xff },
                    ]);
                    raw as f64 / 8_388_608.0
                }
                PcmEncoding::Int24In32 => {
                    let raw = i32::from_le_bytes(
                        sample.try_into().map_err(|_| ConvertError::InvalidFrame)?,
                    ) >> 8;
                    raw as f64 / 8_388_608.0
                }
                PcmEncoding::Int32 => {
                    i32::from_le_bytes(sample.try_into().map_err(|_| ConvertError::InvalidFrame)?)
                        as f64
                        / 2_147_483_648.0
                }
            };
            if !value.is_finite() {
                out.clear();
                return Err(ConvertError::NonFiniteSample);
            }
            sum += value;
        }
        out.push((sum / format.channels as f64) as f32);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn format(channels: u16, encoding: PcmEncoding) -> PcmFormat {
        PcmFormat {
            sample_rate: 48_000,
            channels,
            encoding,
        }
    }

    #[test]
    fn converts_stereo_float_and_opposite_polarity() {
        let bytes = [0.5_f32.to_le_bytes(), (-0.5_f32).to_le_bytes()].concat();
        let mut output = Vec::new();
        to_mono_f32(&bytes, format(2, PcmEncoding::Float32), false, &mut output).unwrap();
        assert_eq!(output, [0.0]);
    }

    #[test]
    fn converts_integer_scales_and_silence() {
        let mut output = Vec::new();
        to_mono_f32(
            &i16::MIN.to_le_bytes(),
            format(1, PcmEncoding::Int16),
            false,
            &mut output,
        )
        .unwrap();
        assert_eq!(output, [-1.0]);
        to_mono_f32(
            &[0xff, 0xff, 0x7f],
            format(1, PcmEncoding::Int24),
            false,
            &mut output,
        )
        .unwrap();
        assert!((output[0] - 1.0).abs() < 0.000001);
        to_mono_f32(
            &i32::MIN.to_le_bytes(),
            format(1, PcmEncoding::Int32),
            false,
            &mut output,
        )
        .unwrap();
        assert_eq!(output, [-1.0]);
        to_mono_f32(
            &[0x00, 0x00, 0x00, 0x80],
            format(1, PcmEncoding::Int24In32),
            false,
            &mut output,
        )
        .unwrap();
        assert_eq!(output, [-1.0]);
        to_mono_f32(
            &[0xff, 0xff, 0xff, 0x7f],
            format(1, PcmEncoding::Int24In32),
            true,
            &mut output,
        )
        .unwrap();
        assert_eq!(output, [0.0]);
    }

    #[test]
    fn rejects_non_finite_and_partial_frames() {
        let mut output = Vec::new();
        assert_eq!(
            to_mono_f32(
                &f32::NAN.to_le_bytes(),
                format(1, PcmEncoding::Float32),
                false,
                &mut output
            ),
            Err(ConvertError::NonFiniteSample)
        );
        assert!(output.is_empty());
        assert_eq!(
            to_mono_f32(&[0], format(2, PcmEncoding::Int16), false, &mut output),
            Err(ConvertError::InvalidFrame)
        );
    }

    #[test]
    fn validates_wave_format() {
        let wave = WaveFormat::new(32, 32, &SampleType::Float, 48_000, 2, None);
        assert_eq!(
            PcmFormat::from_wave_format(&wave).unwrap(),
            format(2, PcmEncoding::Float32)
        );
        let unsupported = WaveFormat::new(64, 64, &SampleType::Float, 48_000, 2, None);
        assert_eq!(
            PcmFormat::from_wave_format(&unsupported),
            Err(ConvertError::UnsupportedFormat)
        );
    }
}
