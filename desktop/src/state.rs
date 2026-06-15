use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

pub use fp_core::{Channel, Filter, Waveform};

pub const MAX_OSCILLATORS: usize = 8;
pub const MIN_FREQ: f64 = 0.01;
pub const MAX_FREQ: f64 = 96_000.0;

// ── Atomic helpers ────────────────────────────────────────────────────────────

fn load_f64(a: &AtomicU64) -> f64 {
    f64::from_bits(a.load(Ordering::Relaxed))
}

fn store_f64(a: &AtomicU64, v: f64) {
    a.store(v.to_bits(), Ordering::Relaxed);
}

// ── OscillatorState ───────────────────────────────────────────────────────────

/// Lock-free oscillator parameters shared between the UI thread and audio thread.
pub struct OscillatorState {
    frequency: AtomicU64, // f64 bits
    amplitude: AtomicU64, // f64 bits [0.0, 1.0]
    waveform:  AtomicU32,
    channel:   AtomicU32,
    filter:    AtomicU32,
    enabled:   AtomicBool,
}

impl OscillatorState {
    pub fn new(freq: f64) -> Self {
        Self {
            frequency: AtomicU64::new(freq.to_bits()),
            amplitude: AtomicU64::new(0.8f64.to_bits()),
            waveform:  AtomicU32::new(0),
            channel:   AtomicU32::new(0),
            filter:    AtomicU32::new(0),
            enabled:   AtomicBool::new(false),
        }
    }

    pub fn get_freq(&self)     -> f64      { load_f64(&self.frequency) }
    pub fn get_amp(&self)      -> f64      { load_f64(&self.amplitude) }
    pub fn get_waveform(&self) -> Waveform { Waveform::from_u32(self.waveform.load(Ordering::Relaxed)) }
    pub fn get_channel(&self)  -> Channel  { Channel::from_u32(self.channel.load(Ordering::Relaxed)) }
    pub fn get_filter(&self)   -> Filter   { Filter::from_u32(self.filter.load(Ordering::Relaxed)) }
    pub fn is_enabled(&self)   -> bool     { self.enabled.load(Ordering::Relaxed) }

    pub fn set_freq(&self, v: f64)      { store_f64(&self.frequency, v.clamp(MIN_FREQ, MAX_FREQ)); }
    pub fn set_amp(&self, v: f64)       { store_f64(&self.amplitude, v.clamp(0.0, 1.0)); }
    pub fn set_waveform(&self, w: Waveform) { self.waveform.store(w as u32, Ordering::Relaxed); }
    pub fn set_channel(&self, c: Channel)   { self.channel.store(c as u32, Ordering::Relaxed); }
    pub fn set_filter(&self, f: Filter)     { self.filter.store(f as u32, Ordering::Relaxed); }
    pub fn set_enabled(&self, e: bool)      { self.enabled.store(e, Ordering::Relaxed); }
}

// ── AppState ──────────────────────────────────────────────────────────────────

pub struct AppState {
    pub oscillators:   Vec<Arc<OscillatorState>>,
    master_volume:     AtomicU64,
    pub playing:       AtomicBool,
    pub osc_count:     AtomicU32,
    /// Device sample rate — written once by AudioEngine, read by file loader.
    pub device_sample_rate: AtomicU64,
    /// Decoded, resampled audio samples from the loaded file.
    /// The audio thread reads these as looping playback when Filter::Custom is active.
    pub file_samples: Mutex<Option<Arc<Vec<f32>>>>,
    /// Display name of the loaded file (filename only).
    pub file_name: Mutex<String>,
}

impl AppState {
    pub fn new() -> Arc<Self> {
        let oscillators: Vec<Arc<OscillatorState>> = (0..MAX_OSCILLATORS)
            .map(|i| {
                let osc = OscillatorState::new(440.0);
                if i == 0 {
                    osc.set_enabled(true);
                }
                Arc::new(osc)
            })
            .collect();

        Arc::new(Self {
            oscillators,
            master_volume:      AtomicU64::new(0.8f64.to_bits()),
            playing:            AtomicBool::new(false),
            osc_count:          AtomicU32::new(1),
            device_sample_rate: AtomicU64::new(44_100f64.to_bits()),
            file_samples:       Mutex::new(None),
            file_name:          Mutex::new(String::new()),
        })
    }

    pub fn get_master_vol(&self) -> f64  { load_f64(&self.master_volume) }
    pub fn set_master_vol(&self, v: f64) { store_f64(&self.master_volume, v.clamp(0.0, 1.0)); }
    pub fn is_playing(&self)    -> bool  { self.playing.load(Ordering::Relaxed) }
    pub fn get_osc_count(&self) -> usize { self.osc_count.load(Ordering::Relaxed) as usize }
    pub fn get_device_sample_rate(&self) -> f64 { load_f64(&self.device_sample_rate) }
    pub fn set_device_sample_rate(&self, sr: f64) { store_f64(&self.device_sample_rate, sr); }

    /// Add a new oscillator. Returns true if successful.
    pub fn add_oscillator(&self) -> bool {
        let count = self.get_osc_count();
        if count >= MAX_OSCILLATORS {
            return false;
        }
        self.oscillators[count].set_freq(440.0);
        self.oscillators[count].set_amp(0.8);
        self.oscillators[count].set_waveform(Waveform::Sine);
        self.oscillators[count].set_channel(Channel::Both);
        self.oscillators[count].set_enabled(true);
        self.osc_count.store((count + 1) as u32, Ordering::Relaxed);
        true
    }

    /// Remove oscillator at `idx`, shifting the rest left. Returns true if successful.
    pub fn remove_oscillator(&self, idx: usize) -> bool {
        let count = self.get_osc_count();
        if count <= 1 || idx >= count {
            return false;
        }
        for i in idx..count - 1 {
            let src = &self.oscillators[i + 1];
            let dst = &self.oscillators[i];
            dst.set_freq(src.get_freq());
            dst.set_amp(src.get_amp());
            dst.set_waveform(src.get_waveform());
            dst.set_channel(src.get_channel());
        }
        self.oscillators[count - 1].set_enabled(false);
        self.osc_count.store((count - 1) as u32, Ordering::Relaxed);
        true
    }
}
