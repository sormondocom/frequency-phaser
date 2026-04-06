//! File-based audio playback with tunable resonance filter.
//!
//! When `Filter::Custom` is active on an oscillator the audio *thread*:
//!   1. Reads the decoded file samples in a looping playback position.
//!   2. Passes each sample through a biquad bandpass filter whose centre
//!      frequency tracks the oscillator's current frequency dial.
//!
//! The result: the file audio plays back, and whichever frequency you tune
//! the oscillator to is emphasised/resonated in the live audio.  Dial in
//! 7.83 Hz on a rain recording to resonate Schumann; dial a bowl's
//! fundamental to bring out its ring; tune a vocal to a solfeggio frequency.
//!
//! Q is fixed at 8 (fairly narrow — about 1.5 semitones either side).
//! Coefficients are recomputed only when the frequency changes by > 0.5 Hz.

use std::f32::consts::PI;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

// ── Decoding ──────────────────────────────────────────────────────────────────

/// Decode a WAV or MP3 file and return `(mono_samples_f32, file_sample_rate)`.
pub fn decode_audio(path: &str) -> anyhow::Result<(Vec<f32>, u32)> {
    use std::fs::File;

    let file = File::open(path)?;
    let mss  = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
    {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe().format(
        &hint, mss,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    )?;

    let mut format = probed.format;

    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| anyhow::anyhow!("No supported audio track in '{}'", path))?
        .clone();

    let track_id    = track.id;
    let n_channels  = track.codec_params.channels.map(|c| c.count()).unwrap_or(1);
    let sample_rate = track.codec_params.sample_rate.unwrap_or(44_100);

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())?;

    let mut raw: Vec<f32>                   = Vec::new();
    let mut sbuf: Option<SampleBuffer<f32>> = None;

    loop {
        let packet = match format.next_packet() {
            Ok(p)  => p,
            Err(_) => break,
        };
        if packet.track_id() != track_id { continue; }

        let decoded = match decoder.decode(&packet) {
            Ok(d)                               => d,
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(_)                              => break,
        };

        let spec     = *decoded.spec();
        let capacity = decoded.capacity() as u64;
        let buf = sbuf.get_or_insert_with(|| SampleBuffer::<f32>::new(capacity, spec));
        buf.copy_interleaved_ref(decoded);
        raw.extend_from_slice(buf.samples());
    }

    if raw.is_empty() {
        anyhow::bail!("File decoded to zero samples");
    }

    // Mix down to mono
    let mono: Vec<f32> = if n_channels <= 1 {
        raw
    } else {
        let ch = n_channels;
        raw.chunks(ch).map(|f| f.iter().sum::<f32>() / ch as f32).collect()
    };

    Ok((mono, sample_rate))
}

/// Linear-interpolation resample from `from_rate` to `to_rate`.
pub fn resample(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate || samples.is_empty() {
        return samples.to_vec();
    }
    let ratio   = from_rate as f64 / to_rate as f64;
    let out_len = ((samples.len() as f64 / ratio).ceil() as usize).max(1);
    (0..out_len)
        .map(|i| {
            let src_f = i as f64 * ratio;
            let src_i = src_f as usize;
            let frac  = (src_f - src_i as f64) as f32;
            let a = samples[src_i.min(samples.len() - 1)];
            let b = samples[(src_i + 1).min(samples.len() - 1)];
            a + (b - a) * frac
        })
        .collect()
}

// ── Peaking EQ biquad (RBJ cookbook) ─────────────────────────────────────────
//
// Unlike a bandpass, a peaking EQ passes the FULL audio signal and adds a
// boost at the centre frequency.  The file plays at full fidelity and the
// dialled frequency resonates on top of it.
//
// For sub-audible centre frequencies (< 20 Hz, e.g. 7.83 Hz Schumann) the
// filter has no audible effect — the file simply plays clean.  That's correct:
// you're tuning the resonance into the material, not gating the playback.

/// +15 dB peak gain at the resonance frequency.
const PEAK_GAIN_DB: f32 = 15.0;
/// Q ≈ 8 — about 1.5 semitones either side of centre.
const PEAK_Q: f32 = 8.0;

pub struct PeakingEq {
    b0: f32, b1: f32, b2: f32,
    a1: f32, a2: f32,
    x1: f32, x2: f32,
    y1: f32, y2: f32,
    last_freq: f32,
    last_sr:   f32,
}

impl PeakingEq {
    pub fn new() -> Self {
        let mut eq = Self {
            b0: 1.0, b1: 0.0, b2: 0.0,
            a1: 0.0, a2: 0.0,
            x1: 0.0, x2: 0.0,
            y1: 0.0, y2: 0.0,
            last_freq: 0.0,
            last_sr:   0.0,
        };
        eq.update_coeffs(440.0, 44_100.0);
        eq
    }

    fn update_coeffs(&mut self, freq: f32, sr: f32) {
        // Clamp to audible range for the peak so sub-Hz dials don't cause instability.
        // The file still plays through clean; the peak just has no audible effect below ~20 Hz.
        let fc    = freq.max(20.0).min(sr * 0.499);
        let w0    = 2.0 * PI * fc / sr;
        let a_lin = 10f32.powf(PEAK_GAIN_DB / 40.0); // sqrt of linear amplitude gain
        let alpha = w0.sin() / (2.0 * PEAK_Q);
        let a0_r  = 1.0 / (1.0 + alpha / a_lin);

        self.b0 = (1.0 + alpha * a_lin) * a0_r;
        self.b1 = (-2.0 * w0.cos())     * a0_r;
        self.b2 = (1.0 - alpha * a_lin) * a0_r;
        self.a1 = (-2.0 * w0.cos())     * a0_r;
        self.a2 = (1.0 - alpha / a_lin) * a0_r;

        self.last_freq = freq;
        self.last_sr   = sr;
    }

    #[inline]
    pub fn process(&mut self, input: f32, freq: f32, sr: f32) -> f32 {
        if (freq - self.last_freq).abs() > 0.5 || (sr - self.last_sr).abs() > 1.0 {
            self.update_coeffs(freq, sr);
        }
        // Direct form I
        let y = self.b0 * input + self.b1 * self.x1 + self.b2 * self.x2
              - self.a1 * self.y1 - self.a2 * self.y2;
        self.x2 = self.x1; self.x1 = input;
        self.y2 = self.y1; self.y1 = y;
        // Soft-clip to prevent runaway resonance at extreme settings
        y.clamp(-1.0, 1.0)
    }

    pub fn reset(&mut self) {
        self.x1 = 0.0; self.x2 = 0.0;
        self.y1 = 0.0; self.y2 = 0.0;
    }
}

// ── Per-oscillator file playback state ────────────────────────────────────────

/// Lives on the audio thread. Holds the looping playback position and
/// the peaking EQ state for one oscillator in Custom filter mode.
pub struct FilePlaybackState {
    pub pos: usize,
    pub eq:  PeakingEq,
}

impl FilePlaybackState {
    pub fn new() -> Self {
        Self {
            pos: 0,
            eq:  PeakingEq::new(),
        }
    }

    /// Return the next sample from `samples` with a resonance peak at `freq`.
    /// The full audio plays through; `freq` is boosted by +15 dB.
    #[inline]
    pub fn tick(&mut self, samples: &[f32], freq: f32, sr: f32) -> f32 {
        if samples.is_empty() { return 0.0; }
        let raw = samples[self.pos % samples.len()];
        self.pos = self.pos.wrapping_add(1);
        self.eq.process(raw, freq, sr)
    }

    pub fn reset(&mut self) {
        self.pos = 0;
        self.eq.reset();
    }
}
