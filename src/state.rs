use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

pub const MAX_OSCILLATORS: usize = 8;
pub const MIN_FREQ: f64 = 0.01;
pub const MAX_FREQ: f64 = 96_000.0;

// ── Waveform ──────────────────────────────────────────────────────────────────

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Waveform {
    Sine     = 0,
    Square   = 1,
    Triangle = 2,
    Sawtooth = 3,
    Pink     = 4,
}

impl Waveform {
    pub fn from_u32(v: u32) -> Self {
        match v {
            1 => Self::Square,
            2 => Self::Triangle,
            3 => Self::Sawtooth,
            4 => Self::Pink,
            _ => Self::Sine,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Sine     => "SINE",
            Self::Square   => "SQR ",
            Self::Triangle => "TRI ",
            Self::Sawtooth => "SAW ",
            Self::Pink     => "PINK",
        }
    }

    pub fn symbol(self) -> &'static str {
        match self {
            Self::Sine     => "∿",
            Self::Square   => "⊓",
            Self::Triangle => "⋀",
            Self::Sawtooth => "⟋",
            Self::Pink     => "≋",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Sine     => Self::Square,
            Self::Square   => Self::Triangle,
            Self::Triangle => Self::Sawtooth,
            Self::Sawtooth => Self::Pink,
            Self::Pink     => Self::Sine,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::Sine     => Self::Pink,
            Self::Square   => Self::Sine,
            Self::Triangle => Self::Square,
            Self::Sawtooth => Self::Triangle,
            Self::Pink     => Self::Sawtooth,
        }
    }

    pub fn all() -> &'static [Waveform] {
        static ALL: [Waveform; 5] = [
            Waveform::Sine,
            Waveform::Square,
            Waveform::Triangle,
            Waveform::Sawtooth,
            Waveform::Pink,
        ];
        &ALL
    }
}

// ── Channel ───────────────────────────────────────────────────────────────────

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Channel {
    Both  = 0,
    Left  = 1,
    Right = 2,
}

impl Channel {
    pub fn from_u32(v: u32) -> Self {
        match v {
            1 => Self::Left,
            2 => Self::Right,
            _ => Self::Both,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Both  => "L+R",
            Self::Left  => "L  ",
            Self::Right => "  R",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Both  => Self::Left,
            Self::Left  => Self::Right,
            Self::Right => Self::Both,
        }
    }

    pub fn all() -> &'static [Channel] {
        static ALL: [Channel; 3] = [Channel::Both, Channel::Left, Channel::Right];
        &ALL
    }
}

// ── Filter ────────────────────────────────────────────────────────────────────

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Filter {
    None       = 0,
    Orchestral = 1,
    Choir      = 2,
    BassDrum   = 3,
    Shofar     = 4,
    /// FIR filter built from a user-loaded WAV/MP3 file.
    Custom     = 5,
}

impl Filter {
    pub fn from_u32(v: u32) -> Self {
        match v {
            1 => Self::Orchestral,
            2 => Self::Choir,
            3 => Self::BassDrum,
            4 => Self::Shofar,
            5 => Self::Custom,
            _ => Self::None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::None       => "RAW",
            Self::Orchestral => "ORCH",
            Self::Choir      => "CHOIR",
            Self::BassDrum   => "DRUM",
            Self::Shofar     => "SHOFAR",
            Self::Custom     => "FILE",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::None       => "No filter",
            Self::Orchestral => "String symphony",
            Self::Choir      => "Angelic choir",
            Self::BassDrum   => "Tribal bass drum",
            Self::Shofar     => "Hebrew shofar",
            Self::Custom     => "Custom file filter",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::None       => Self::Orchestral,
            Self::Orchestral => Self::Choir,
            Self::Choir      => Self::BassDrum,
            Self::BassDrum   => Self::Shofar,
            Self::Shofar     => Self::Custom,
            Self::Custom     => Self::None,
        }
    }

    pub fn all() -> &'static [Filter] {
        static ALL: [Filter; 6] = [
            Filter::None, Filter::Orchestral, Filter::Choir,
            Filter::BassDrum, Filter::Shofar, Filter::Custom,
        ];
        &ALL
    }
}

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
