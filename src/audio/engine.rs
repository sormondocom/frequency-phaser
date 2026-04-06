use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
use std::sync::Arc;

use crate::state::{AppState, Channel, MAX_OSCILLATORS};
use super::generator::OscillatorRt;

pub struct AudioEngine {
    _stream:     Stream,
    pub sample_rate:  f64,
    pub device_name:  String,
    pub channel_count: usize,
}

impl AudioEngine {
    pub fn new(state: Arc<AppState>) -> Result<Self> {
        let host   = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| anyhow!("No audio output device found"))?;

        let device_name = device.name().unwrap_or_else(|_| "Unknown".into());
        let supported   = device.default_output_config()?;
        let sample_rate = supported.sample_rate().0 as f64;
        let channels    = supported.channels() as usize;

        let stream = match supported.sample_format() {
            SampleFormat::I8  => make_stream::<i8>( &device, &supported.into(), Arc::clone(&state), sample_rate, channels),
            SampleFormat::I16 => make_stream::<i16>(&device, &supported.into(), Arc::clone(&state), sample_rate, channels),
            SampleFormat::I32 => make_stream::<i32>(&device, &supported.into(), Arc::clone(&state), sample_rate, channels),
            SampleFormat::I64 => make_stream::<i64>(&device, &supported.into(), Arc::clone(&state), sample_rate, channels),
            SampleFormat::U8  => make_stream::<u8>( &device, &supported.into(), Arc::clone(&state), sample_rate, channels),
            SampleFormat::U16 => make_stream::<u16>(&device, &supported.into(), Arc::clone(&state), sample_rate, channels),
            SampleFormat::U32 => make_stream::<u32>(&device, &supported.into(), Arc::clone(&state), sample_rate, channels),
            SampleFormat::U64 => make_stream::<u64>(&device, &supported.into(), Arc::clone(&state), sample_rate, channels),
            SampleFormat::F32 => make_stream::<f32>(&device, &supported.into(), Arc::clone(&state), sample_rate, channels),
            SampleFormat::F64 => make_stream::<f64>(&device, &supported.into(), Arc::clone(&state), sample_rate, channels),
            fmt => return Err(anyhow!("Unsupported sample format: {:?}", fmt)),
        }?;

        stream.play()?;

        Ok(Self {
            _stream: stream,
            sample_rate,
            device_name,
            channel_count: channels,
        })
    }
}

fn make_stream<T>(
    device:      &cpal::Device,
    config:      &cpal::StreamConfig,
    state:       Arc<AppState>,
    sample_rate: f64,
    channels:    usize,
) -> Result<Stream>
where
    T: cpal::Sample + cpal::SizedSample + cpal::FromSample<f32>,
{
    let mut oscillators: Vec<OscillatorRt> = (0..MAX_OSCILLATORS).map(|_| OscillatorRt::new()).collect();

    let stream = device.build_output_stream(
        config,
        move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
            if !state.is_playing() {
                for s in data.iter_mut() {
                    *s = T::from_sample(0.0f32);
                }
                return;
            }

            let master_vol = state.get_master_vol();
            let osc_count  = state.get_osc_count();

            for frame in data.chunks_mut(channels) {
                let mut left   = 0.0f64;
                let mut right  = 0.0f64;
                let mut active = 0usize;

                for i in 0..osc_count {
                    let osc_s = &state.oscillators[i];
                    if !osc_s.is_enabled() {
                        // Advance phases to keep timing consistent when re-enabled
                        let _ = oscillators[i].tick(osc_s.get_waveform(), osc_s.get_filter(), osc_s.get_freq(), sample_rate);
                        continue;
                    }

                    let freq     = osc_s.get_freq();
                    let amp      = osc_s.get_amp();
                    let waveform = osc_s.get_waveform();
                    let filter   = osc_s.get_filter();
                    let chan      = osc_s.get_channel();

                    let s = oscillators[i].tick(waveform, filter, freq, sample_rate) * amp;

                    match chan {
                        Channel::Both  => { left += s; right += s; }
                        Channel::Left  => { left  += s; }
                        Channel::Right => { right += s; }
                    }
                    active += 1;
                }

                // Soft normalise to prevent clipping with many oscillators
                let norm = if active > 1 { 1.0 / (active as f64).sqrt() } else { 1.0 };
                let l = ((left  * norm * master_vol) as f32).clamp(-1.0, 1.0);
                let r = ((right * norm * master_vol) as f32).clamp(-1.0, 1.0);

                if channels >= 2 {
                    frame[0] = T::from_sample(l);
                    frame[1] = T::from_sample(r);
                    for s in frame[2..].iter_mut() {
                        *s = T::from_sample(0.0f32);
                    }
                } else {
                    frame[0] = T::from_sample((l + r) * 0.5);
                }
            }
        },
        |err| eprintln!("Audio stream error: {err}"),
        None,
    )?;

    Ok(stream)
}
