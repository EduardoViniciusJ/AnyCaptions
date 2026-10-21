use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use anycaptions_lib::audio::{
    capture::{CaptureEvent, CaptureSession, WasapiLoopback},
    convert::to_mono_f32,
    devices::{AudioDeviceCatalog, WasapiCatalog},
    resample::MonoResampler,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let selected_id = if let Some(id) = std::env::args().nth(1) {
        id
    } else {
        let inventory = WasapiCatalog
            .scan()
            .map_err(|e| format!("Cannot list outputs: {e:?}"))?;
        inventory.default_id.ok_or("No default output")?
    };
    for cycle in 1..=3 {
        let mut session = CaptureSession::start(Arc::new(WasapiLoopback), selected_id.clone())
            .map_err(|e| format!("Cannot start loopback: {e:?}"))?;
        let format = session.format();
        let mut resampler = MonoResampler::new(format.sample_rate)
            .map_err(|e| format!("Cannot resample: {e:?}"))?;
        let mut mono = Vec::new();
        let mut output = Vec::new();
        let mut packets = 0_u32;
        let mut silent = 0_u32;
        let mut frames = 0_usize;
        let mut audible_frames = 0_usize;
        let until = Instant::now() + Duration::from_secs(2);
        while Instant::now() < until {
            match session.recv_timeout(Duration::from_millis(200)) {
                Ok(CaptureEvent::Packet(packet)) => {
                    if packet.discontinuity {
                        resampler.reset();
                    }
                    to_mono_f32(&packet.bytes, format, packet.silent, &mut mono)
                        .map_err(|e| format!("Cannot convert packet: {e:?}"))?;
                    output.clear();
                    for chunk in mono.chunks(65_536) {
                        resampler
                            .push(chunk, &mut output)
                            .map_err(|e| format!("Cannot resample packet: {e:?}"))?;
                    }
                    frames += output.len();
                    audible_frames += output.iter().filter(|sample| sample.abs() > 0.001).count();
                    packets += 1;
                    silent += u32::from(packet.silent);
                }
                Ok(CaptureEvent::Stopped(error)) => {
                    return Err(format!("Capture stopped: {error:?}").into());
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => (),
                Err(error) => return Err(format!("Capture channel closed: {error}").into()),
            }
        }
        session
            .stop()
            .map_err(|e| format!("Cannot stop capture: {e:?}"))?;
        println!(
            "Cycle {cycle}: {} Hz, {} channels, {packets} packets, {silent} silent packets, {frames} frames at 16 kHz, {audible_frames} above threshold",
            format.sample_rate, format.channels
        );
    }
    Ok(())
}
