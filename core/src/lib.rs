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

// ── Preset ────────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct Preset {
    pub name:     &'static str,
    pub freq:     f64,
    pub waveform: Waveform,
    pub category: &'static str,
}

impl Preset {
    const fn new(
        name:     &'static str,
        freq:     f64,
        waveform: Waveform,
        category: &'static str,
    ) -> Self {
        Self { name, freq, waveform, category }
    }
}

pub const PRESETS: &[Preset] = &[
    // ── Schumann Resonances ───────────────────────────────────────────────────
    Preset::new("Schumann 1st",    7.83,  Waveform::Sine, "Schumann"),
    Preset::new("Schumann 2nd",   14.30,  Waveform::Sine, "Schumann"),
    Preset::new("Schumann 3rd",   20.80,  Waveform::Sine, "Schumann"),
    Preset::new("Schumann 4th",   27.30,  Waveform::Sine, "Schumann"),
    Preset::new("Schumann 5th",   33.80,  Waveform::Sine, "Schumann"),

    // ── Brainwave Entrainment ─────────────────────────────────────────────────
    Preset::new("Delta",           2.00,  Waveform::Sine, "Brainwave"),
    Preset::new("Theta",           6.00,  Waveform::Sine, "Brainwave"),
    Preset::new("Alpha",          10.00,  Waveform::Sine, "Brainwave"),
    Preset::new("SMR",            12.00,  Waveform::Sine, "Brainwave"),
    Preset::new("Beta",           20.00,  Waveform::Sine, "Brainwave"),
    Preset::new("Gamma",          40.00,  Waveform::Sine, "Brainwave"),
    Preset::new("Hi-Gamma",      100.00,  Waveform::Sine, "Brainwave"),

    // ── Solfeggio Frequencies ─────────────────────────────────────────────────
    Preset::new("UT  174 Hz",    174.0,   Waveform::Sine, "Solfeggio"),
    Preset::new("RE  285 Hz",    285.0,   Waveform::Sine, "Solfeggio"),
    Preset::new("UT  396 Hz",    396.0,   Waveform::Sine, "Solfeggio"),
    Preset::new("RE  417 Hz",    417.0,   Waveform::Sine, "Solfeggio"),
    Preset::new("MI  528 Hz",    528.0,   Waveform::Sine, "Solfeggio"),
    Preset::new("FA  639 Hz",    639.0,   Waveform::Sine, "Solfeggio"),
    Preset::new("SOL 741 Hz",    741.0,   Waveform::Sine, "Solfeggio"),
    Preset::new("LA  852 Hz",    852.0,   Waveform::Sine, "Solfeggio"),
    Preset::new("SI  963 Hz",    963.0,   Waveform::Sine, "Solfeggio"),

    // ── Chakra Healing ────────────────────────────────────────────────────────
    Preset::new("Root     194.18 Hz",  194.18,  Waveform::Sine, "Chakra"),
    Preset::new("Sacral   210.42 Hz",  210.42,  Waveform::Sine, "Chakra"),
    Preset::new("Solar    126.22 Hz",  126.22,  Waveform::Sine, "Chakra"),
    Preset::new("Heart    136.10 Hz",  136.10,  Waveform::Sine, "Chakra"),
    Preset::new("Throat   141.27 Hz",  141.27,  Waveform::Sine, "Chakra"),
    Preset::new("Third Eye 221.23 Hz", 221.23,  Waveform::Sine, "Chakra"),
    Preset::new("Crown    172.06 Hz",  172.06,  Waveform::Sine, "Chakra"),
    Preset::new("Root     396 Hz",     396.0,   Waveform::Sine, "Chakra"),
    Preset::new("Sacral   417 Hz",     417.0,   Waveform::Sine, "Chakra"),
    Preset::new("Solar    528 Hz",     528.0,   Waveform::Sine, "Chakra"),
    Preset::new("Heart    639 Hz",     639.0,   Waveform::Sine, "Chakra"),
    Preset::new("Throat   741 Hz",     741.0,   Waveform::Sine, "Chakra"),
    Preset::new("Third Eye 852 Hz",    852.0,   Waveform::Sine, "Chakra"),
    Preset::new("Crown    963 Hz",     963.0,   Waveform::Sine, "Chakra"),

    // ── Musical Reference ─────────────────────────────────────────────────────
    Preset::new("Concert A   440 Hz",  440.0,   Waveform::Sine, "Musical"),
    Preset::new("A432",                432.0,   Waveform::Sine, "Musical"),
    Preset::new("C4 Middle C",         261.63,  Waveform::Sine, "Musical"),
    Preset::new("A3",                  220.0,   Waveform::Sine, "Musical"),
    Preset::new("A5",                  880.0,   Waveform::Sine, "Musical"),

    // ── Healing / Resonance ───────────────────────────────────────────────────
    Preset::new("528 Hz DNA Repair",   528.0,   Waveform::Sine, "Healing"),
    Preset::new("Earth (Schumann)",      7.83,  Waveform::Sine, "Healing"),
    Preset::new("Om (Earth Year)",     136.10,  Waveform::Sine, "Healing"),

    // ── Geotechnical ─────────────────────────────────────────────────────────
    Preset::new("Sandstone  ~10 cm",   12_500.0, Waveform::Sine, "Geotechnical"),
    Preset::new("Shale      ~10 cm",    9_000.0, Waveform::Sine, "Geotechnical"),
    Preset::new("Limestone  ~10 cm",   27_500.0, Waveform::Sine, "Geotechnical"),
    Preset::new("Granite    ~10 cm",   30_000.0, Waveform::Sine, "Geotechnical"),
    Preset::new("Sandstone  ~50 cm",    2_500.0, Waveform::Sine, "Geotechnical"),
    Preset::new("Shale      ~50 cm",    1_800.0, Waveform::Sine, "Geotechnical"),
    Preset::new("Limestone  ~50 cm",    5_500.0, Waveform::Sine, "Geotechnical"),
    Preset::new("Granite    ~50 cm",    6_000.0, Waveform::Sine, "Geotechnical"),
    Preset::new("Sandstone  ~1 m",      1_250.0, Waveform::Sine, "Geotechnical"),
    Preset::new("Shale      ~1 m",        900.0, Waveform::Sine, "Geotechnical"),
    Preset::new("Limestone  ~1 m",      2_750.0, Waveform::Sine, "Geotechnical"),
    Preset::new("Granite    ~1 m",      3_000.0, Waveform::Sine, "Geotechnical"),
    Preset::new("Sonic drill fund.",       80.0, Waveform::Sine, "Geotechnical"),
    Preset::new("Seismic explore low",     10.0, Waveform::Sine, "Geotechnical"),
    Preset::new("Clay S-wave low",        100.0, Waveform::Sine, "Geotechnical"),
    Preset::new("Clay S-wave high",       300.0, Waveform::Sine, "Geotechnical"),

    // ── Reference Boundaries ─────────────────────────────────────────────────
    Preset::new("Infrasound top",       20.0,   Waveform::Sine, "Reference"),
    Preset::new("Ultrasound low",    20_000.0,  Waveform::Sine, "Reference"),
    Preset::new("Sub 1 Hz",              1.0,   Waveform::Sine, "Reference"),
    Preset::new("Sub 5 Hz",              5.0,   Waveform::Sine, "Reference"),
    Preset::new("10 Hz",                10.0,   Waveform::Sine, "Reference"),
    Preset::new("100 Hz",              100.0,   Waveform::Sine, "Reference"),
    Preset::new("1 kHz",             1_000.0,   Waveform::Sine, "Reference"),
    Preset::new("10 kHz",           10_000.0,   Waveform::Sine, "Reference"),
];

/// Returns unique category names in order of first appearance.
pub fn categories() -> Vec<&'static str> {
    let mut seen = Vec::new();
    for p in PRESETS {
        if !seen.contains(&p.category) {
            seen.push(p.category);
        }
    }
    seen
}

// ── Oscillator engine ─────────────────────────────────────────────────────────

use std::f64::consts::PI;
const TWO_PI: f64 = 2.0 * PI;

/// Paul Kellet's pink noise approximation (xorshift64 white source).
pub struct PinkNoiseGen {
    b:   [f64; 7],
    rng: u64,
}

impl PinkNoiseGen {
    pub fn new() -> Self {
        Self { b: [0.0; 7], rng: 0xdead_beef_cafe_babe }
    }

    fn white(&mut self) -> f64 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng as i64 as f64) / (i64::MAX as f64)
    }

    pub fn next(&mut self) -> f64 {
        let w = self.white();
        self.b[0] =  0.99886 * self.b[0] + w * 0.0555179;
        self.b[1] =  0.99332 * self.b[1] + w * 0.0750759;
        self.b[2] =  0.96900 * self.b[2] + w * 0.1538520;
        self.b[3] =  0.86650 * self.b[3] + w * 0.3104856;
        self.b[4] =  0.55000 * self.b[4] + w * 0.5329522;
        self.b[5] = -0.76160 * self.b[5] - w * 0.0168980;
        let pink = self.b[0] + self.b[1] + self.b[2] + self.b[3]
                 + self.b[4] + self.b[5] + self.b[6] + w * 0.5362;
        self.b[6] = w * 0.115926;
        pink * 0.11
    }
}

/// Single oscillator with a persistent phase accumulator.
pub struct Oscillator {
    pub phase: f64,
    pub pink:  PinkNoiseGen,
}

impl Oscillator {
    pub fn new() -> Self {
        Self { phase: 0.0, pink: PinkNoiseGen::new() }
    }

    pub fn tick(&mut self, waveform: Waveform, freq: f64, sample_rate: f64) -> f64 {
        let noise = if waveform == Waveform::Pink { self.pink.next() } else { 0.0 };
        let s = match waveform {
            Waveform::Sine     => self.phase.sin(),
            Waveform::Square   => if self.phase < PI { 1.0 } else { -1.0 },
            Waveform::Triangle => {
                if self.phase < PI { -1.0 + 2.0 * self.phase / PI }
                else               {  3.0 - 2.0 * self.phase / PI }
            }
            Waveform::Sawtooth => self.phase / PI - 1.0,
            Waveform::Pink     => noise,
        };
        self.phase += TWO_PI * freq / sample_rate;
        if self.phase >= TWO_PI { self.phase -= TWO_PI; }
        s
    }
}

const CENTS_7_UP: f64 = 1.004_040_578;
const CENTS_7_DN: f64 = 1.0 / CENTS_7_UP;
const HARM_AMPS:  [f64; 6] = [0.60, 0.40, 0.25, 0.15, 0.10, 0.06];
const ORCH_NORM:  f64 = 1.0 / 1.36;

pub struct OrchestrialState {
    fundamental_phase: f64,
    harmonic_phases:   [f64; 6],
    ensemble_phases:   [f64; 2],
    vibrato_phase:     f64,
    noise:             PinkNoiseGen,
}

impl OrchestrialState {
    pub fn new() -> Self {
        Self {
            fundamental_phase: 0.0,
            harmonic_phases:   [0.0; 6],
            ensemble_phases:   [0.0; 2],
            vibrato_phase:     0.0,
            noise:             PinkNoiseGen::new(),
        }
    }

    pub fn tick(&mut self, freq: f64, sample_rate: f64) -> f64 {
        const VIBRATO_RATE: f64  = 5.5;
        const VIBRATO_DEPTH: f64 = 0.003;
        let vibrato = 1.0 + VIBRATO_DEPTH * self.vibrato_phase.sin();
        self.vibrato_phase = (self.vibrato_phase + TWO_PI * VIBRATO_RATE / sample_rate) % TWO_PI;
        let vfreq = freq * vibrato;
        let fund = self.fundamental_phase.sin();
        self.fundamental_phase = (self.fundamental_phase + TWO_PI * vfreq / sample_rate) % TWO_PI;
        let mut harmonics = 0.0f64;
        for i in 0..6 {
            harmonics += HARM_AMPS[i] * self.harmonic_phases[i].sin();
            self.harmonic_phases[i] = (self.harmonic_phases[i] + TWO_PI * vfreq * (i as f64 + 2.0) / sample_rate) % TWO_PI;
        }
        let ens = (self.ensemble_phases[0].sin() + self.ensemble_phases[1].sin()) * 0.3;
        self.ensemble_phases[0] = (self.ensemble_phases[0] + TWO_PI * vfreq * CENTS_7_UP / sample_rate) % TWO_PI;
        self.ensemble_phases[1] = (self.ensemble_phases[1] + TWO_PI * vfreq * CENTS_7_DN / sample_rate) % TWO_PI;
        let noise = self.noise.next() * 0.02;
        (fund + harmonics + ens + noise) * ORCH_NORM
    }
}

const CHOIR_HARM_AMPS: [f64; 10] = [0.80, 0.85, 0.95, 0.75, 0.55, 0.60, 0.40, 0.50, 0.35, 0.20];
const CHOIR_DETUNE:    [f64; 4]  = [1.002_893_56, 0.997_113_28, 1.006_956_53, 0.993_082_51];
const CHOIR_NORM:      f64 = 1.0 / 2.15;

pub struct ChoirState {
    harm_phases:     [f64; 10],
    ensemble_phases: [f64; 4],
    vibrato_phase:   f64,
    noise:           PinkNoiseGen,
}

impl ChoirState {
    pub fn new() -> Self {
        Self {
            harm_phases:     [0.0; 10],
            ensemble_phases: [0.0; 4],
            vibrato_phase:   0.0,
            noise:           PinkNoiseGen::new(),
        }
    }

    pub fn tick(&mut self, freq: f64, sample_rate: f64) -> f64 {
        const VIBRATO_RATE:  f64 = 4.5;
        const VIBRATO_DEPTH: f64 = 0.004;
        const ENSEMBLE_AMP:  f64 = 0.30;
        const NYQUIST_GUARD: f64 = 0.46;
        let vibrato = 1.0 + VIBRATO_DEPTH * self.vibrato_phase.sin();
        self.vibrato_phase = (self.vibrato_phase + TWO_PI * VIBRATO_RATE / sample_rate) % TWO_PI;
        let vfreq = freq * vibrato;
        let nyquist_limit = sample_rate * NYQUIST_GUARD;
        let mut harmonics = 0.0f64;
        for i in 0..10 {
            let hfreq = vfreq * (i as f64 + 1.0);
            if hfreq < nyquist_limit {
                harmonics += CHOIR_HARM_AMPS[i] * self.harm_phases[i].sin();
            }
            self.harm_phases[i] = (self.harm_phases[i] + TWO_PI * hfreq / sample_rate) % TWO_PI;
        }
        let mut ensemble = 0.0f64;
        for i in 0..4 {
            ensemble += self.ensemble_phases[i].sin();
            self.ensemble_phases[i] = (self.ensemble_phases[i] + TWO_PI * vfreq * CHOIR_DETUNE[i] / sample_rate) % TWO_PI;
        }
        let breath = self.noise.next() * 0.03;
        (harmonics + ensemble * ENSEMBLE_AMP + breath) * CHOIR_NORM
    }
}

const DRUM_NORM: f64 = 1.0 / 1.10;

pub struct BassDrumState {
    sub_phase:  f64,
    fund_phase: f64,
    h2_phase:   f64,
    h3_phase:   f64,
    trem_phase: f64,
    noise:      PinkNoiseGen,
}

impl BassDrumState {
    pub fn new() -> Self {
        Self { sub_phase: 0.0, fund_phase: 0.0, h2_phase: 0.0, h3_phase: 0.0, trem_phase: 0.0, noise: PinkNoiseGen::new() }
    }

    pub fn tick(&mut self, freq: f64, sample_rate: f64) -> f64 {
        const TREM_RATE:  f64 = 5.0;
        const TREM_DEPTH: f64 = 0.30;
        let envelope = 1.0 - TREM_DEPTH + TREM_DEPTH * self.trem_phase.sin().abs();
        self.trem_phase = (self.trem_phase + TWO_PI * TREM_RATE / sample_rate) % TWO_PI;
        let sub = self.sub_phase.sin() * 0.55;
        self.sub_phase = (self.sub_phase + TWO_PI * (freq * 0.5) / sample_rate) % TWO_PI;
        let fund = self.fund_phase.sin();
        self.fund_phase = (self.fund_phase + TWO_PI * freq / sample_rate) % TWO_PI;
        let h2 = self.h2_phase.sin() * 0.60;
        self.h2_phase = (self.h2_phase + TWO_PI * freq * 2.0 / sample_rate) % TWO_PI;
        let h3 = self.h3_phase.sin() * 0.25;
        self.h3_phase = (self.h3_phase + TWO_PI * freq * 3.0 / sample_rate) % TWO_PI;
        let skin = self.noise.next() * 0.08;
        (sub + fund + h2 + h3 + skin) * envelope * DRUM_NORM
    }
}

const SHOFAR_HARM_AMPS: [f64; 10] = [0.60, 0.50, 0.90, 0.30, 0.80, 0.25, 0.70, 0.20, 0.50, 0.15];
const SHOFAR_NORM:      f64 = 1.0 / 1.75;

pub struct ShofarState {
    harm_phases:   [f64; 10],
    vibrato_phase: f64,
    noise:         PinkNoiseGen,
}

impl ShofarState {
    pub fn new() -> Self {
        Self { harm_phases: [0.0; 10], vibrato_phase: 0.0, noise: PinkNoiseGen::new() }
    }

    pub fn tick(&mut self, freq: f64, sample_rate: f64) -> f64 {
        const VIBRATO_RATE:  f64 = 6.0;
        const VIBRATO_DEPTH: f64 = 0.008;
        const NYQUIST_GUARD: f64 = 0.46;
        let vibrato = 1.0 + VIBRATO_DEPTH * self.vibrato_phase.sin();
        self.vibrato_phase = (self.vibrato_phase + TWO_PI * VIBRATO_RATE / sample_rate) % TWO_PI;
        let vfreq = freq * vibrato;
        let nyquist_limit = sample_rate * NYQUIST_GUARD;
        let mut harmonics = 0.0f64;
        for i in 0..10 {
            let hfreq = vfreq * (i as f64 + 1.0);
            if hfreq < nyquist_limit {
                harmonics += SHOFAR_HARM_AMPS[i] * self.harm_phases[i].sin();
            }
            self.harm_phases[i] = (self.harm_phases[i] + TWO_PI * hfreq / sample_rate) % TWO_PI;
        }
        let breath = self.noise.next() * 0.04;
        (harmonics + breath) * SHOFAR_NORM
    }
}

/// Per-oscillator runtime state for the ESP32 (no file playback).
/// Bundles the base oscillator with all filter states.
pub struct OscillatorRt {
    pub base:       Oscillator,
    pub orchestral: OrchestrialState,
    pub choir:      ChoirState,
    pub bass_drum:  BassDrumState,
    pub shofar:     ShofarState,
}

impl OscillatorRt {
    pub fn new() -> Self {
        Self {
            base:       Oscillator::new(),
            orchestral: OrchestrialState::new(),
            choir:      ChoirState::new(),
            bass_drum:  BassDrumState::new(),
            shofar:     ShofarState::new(),
        }
    }

    pub fn tick(&mut self, waveform: Waveform, filter: Filter, freq: f64, sample_rate: f64) -> f64 {
        let base_sample = self.base.tick(waveform, freq, sample_rate);
        match filter {
            Filter::None       => base_sample,
            Filter::Orchestral => self.orchestral.tick(freq, sample_rate),
            Filter::Choir      => self.choir.tick(freq, sample_rate),
            Filter::BassDrum   => self.bass_drum.tick(freq, sample_rate),
            Filter::Shofar     => self.shofar.tick(freq, sample_rate),
            Filter::Custom     => 0.0, // no file playback on embedded
        }
    }
}
