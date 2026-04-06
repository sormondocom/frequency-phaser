use crate::state::{Filter, Waveform};
use std::f64::consts::PI;

const TWO_PI: f64 = 2.0 * PI;

// ── Pink noise ────────────────────────────────────────────────────────────────

/// Paul Kellet's pink noise approximation.
pub struct PinkNoiseGen {
    b:   [f64; 7],
    rng: u64,
}

impl PinkNoiseGen {
    pub fn new() -> Self {
        Self {
            b:   [0.0; 7],
            rng: 0xdead_beef_cafe_babe,
        }
    }

    fn white(&mut self) -> f64 {
        // xorshift64
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
        pink * 0.11 // normalise to roughly ±1
    }
}

// ── Oscillator ────────────────────────────────────────────────────────────────

/// Single oscillator with a persistent phase accumulator.
pub struct Oscillator {
    pub phase: f64,
    pub pink:  PinkNoiseGen,
}

impl Oscillator {
    pub fn new() -> Self {
        Self {
            phase: 0.0,
            pink:  PinkNoiseGen::new(),
        }
    }

    /// Generate the next sample and advance the phase.
    pub fn tick(&mut self, waveform: Waveform, freq: f64, sample_rate: f64) -> f64 {
        let s = Self::sample_at(self.phase, waveform);
        let noise = if waveform == Waveform::Pink {
            self.pink.next()
        } else {
            // phase advance still needs to happen even for non-pink
            0.0
        };

        self.phase += TWO_PI * freq / sample_rate;
        if self.phase >= TWO_PI {
            self.phase -= TWO_PI;
        }

        if waveform == Waveform::Pink { noise } else { s }
    }

    /// Stateless sample at a given phase (used by the scope renderer).
    pub fn sample_at(phase: f64, waveform: Waveform) -> f64 {
        match waveform {
            Waveform::Sine     => phase.sin(),
            Waveform::Square   => if phase < PI { 1.0 } else { -1.0 },
            Waveform::Triangle => {
                if phase < PI {
                    -1.0 + 2.0 * phase / PI
                } else {
                    3.0 - 2.0 * phase / PI
                }
            }
            Waveform::Sawtooth => phase / PI - 1.0,
            Waveform::Pink     => 0.0, // non-deterministic; scope shows flat
        }
    }
}

// ── Orchestral filter ─────────────────────────────────────────────────────────
//
// Additive synthesis modelling a bowed string section:
//   • Fundamental sine at the target frequency
//   • Harmonics 2–7 weighted by a string-like spectral envelope
//   • Two ensemble voices detuned ±7 cents (chorus/section width)
//   • 5.5 Hz vibrato LFO at 0.3 % depth
//   • 2 % bow-noise (pink) for texture
//
// All phase accumulators live entirely in the audio thread — no locks needed.

/// 2^(7/1200) — frequency ratio for +7 cents
const CENTS_7_UP: f64 = 1.004_040_578;
/// 1 / 2^(7/1200) — frequency ratio for −7 cents
const CENTS_7_DN: f64 = 1.0 / CENTS_7_UP;

/// Harmonic amplitudes for harmonics 2–7 relative to the fundamental.
const HARM_AMPS: [f64; 6] = [0.60, 0.40, 0.25, 0.15, 0.10, 0.06];

/// Pre-computed RMS normalisation for the full mix:
///   sqrt(1² + 0.6² + 0.4² + 0.25² + 0.15² + 0.10² + 0.06²
///        + (0.3·1)² + (0.3·1)²) ≈ 1.36
const ORCH_NORM: f64 = 1.0 / 1.36;

pub struct OrchestrialState {
    fundamental_phase: f64,
    harmonic_phases:   [f64; 6], // harmonics 2..=7
    ensemble_phases:   [f64; 2], // [+7 cents, −7 cents]
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
        const VIBRATO_RATE:  f64 = 5.5;   // Hz
        const VIBRATO_DEPTH: f64 = 0.003; // ±0.3 % frequency deviation

        // Vibrato LFO
        let vibrato = 1.0 + VIBRATO_DEPTH * self.vibrato_phase.sin();
        self.vibrato_phase = (self.vibrato_phase + TWO_PI * VIBRATO_RATE / sample_rate) % TWO_PI;

        let vfreq = freq * vibrato;

        // Fundamental
        let fund = self.fundamental_phase.sin();
        self.fundamental_phase = (self.fundamental_phase + TWO_PI * vfreq / sample_rate) % TWO_PI;

        // Harmonics 2–7
        let mut harmonics = 0.0f64;
        for i in 0..6 {
            harmonics += HARM_AMPS[i] * self.harmonic_phases[i].sin();
            let hfreq = vfreq * (i as f64 + 2.0);
            self.harmonic_phases[i] = (self.harmonic_phases[i] + TWO_PI * hfreq / sample_rate) % TWO_PI;
        }

        // Ensemble voices (±7 cents, each weighted 0.3)
        let ens = (self.ensemble_phases[0].sin() + self.ensemble_phases[1].sin()) * 0.3;
        self.ensemble_phases[0] = (self.ensemble_phases[0] + TWO_PI * vfreq * CENTS_7_UP / sample_rate) % TWO_PI;
        self.ensemble_phases[1] = (self.ensemble_phases[1] + TWO_PI * vfreq * CENTS_7_DN / sample_rate) % TWO_PI;

        // Bow noise (2 %)
        let noise = self.noise.next() * 0.02;

        (fund + harmonics + ens + noise) * ORCH_NORM
    }
}

// ── Angelic Choir filter ──────────────────────────────────────────────────────
//
// Additive synthesis modelling a soprano section singing "aah":
//   • 10 harmonics weighted by a vowel-formant-like spectral envelope
//   • 4 ensemble voices detuned ±5 and ±12 cents (section width)
//   • 4.5 Hz vibrato LFO at 0.4 % depth (slower, more breath-like than strings)
//   • 3 % breath noise
//   • Harmonics above Nyquist/2 are skipped to prevent aliasing

/// Harmonic amplitudes H1–H10 shaped to approximate an "aah" vowel formant.
/// Peaks around H3 (F1 region) and H8 (F2 region) give the choral vowel colour.
const CHOIR_HARM_AMPS: [f64; 10] =
    [0.80, 0.85, 0.95, 0.75, 0.55, 0.60, 0.40, 0.50, 0.35, 0.20];

/// Cent detuning ratios for the four ensemble voices (±5 and ±12 cents).
const CHOIR_DETUNE: [f64; 4] = [
    1.002_893_56,  // +5 cents
    0.997_113_28,  // −5 cents
    1.006_956_53,  // +12 cents
    0.993_082_51,  // −12 cents
];

/// Approximate RMS normalisation: sqrt(sum(H_i²) + 4·0.3²) ≈ 2.15
const CHOIR_NORM: f64 = 1.0 / 2.15;

pub struct ChoirState {
    harm_phases:     [f64; 10], // centre-voice harmonics 1..=10
    ensemble_phases: [f64; 4],  // detuned voices (fundamental only)
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
        const NYQUIST_GUARD: f64 = 0.46; // headroom below Nyquist

        let vibrato = 1.0 + VIBRATO_DEPTH * self.vibrato_phase.sin();
        self.vibrato_phase = (self.vibrato_phase + TWO_PI * VIBRATO_RATE / sample_rate) % TWO_PI;
        let vfreq = freq * vibrato;

        // Centre voice — 10 harmonics, formant-weighted
        let mut harmonics = 0.0f64;
        let nyquist_limit = sample_rate * NYQUIST_GUARD;
        for i in 0..10 {
            let hfreq = vfreq * (i as f64 + 1.0);
            if hfreq < nyquist_limit {
                harmonics += CHOIR_HARM_AMPS[i] * self.harm_phases[i].sin();
            }
            self.harm_phases[i] = (self.harm_phases[i] + TWO_PI * hfreq / sample_rate) % TWO_PI;
        }

        // Ensemble voices — four detuned copies at the fundamental only
        let mut ensemble = 0.0f64;
        for i in 0..4 {
            ensemble += self.ensemble_phases[i].sin();
            let efreq = vfreq * CHOIR_DETUNE[i];
            self.ensemble_phases[i] = (self.ensemble_phases[i] + TWO_PI * efreq / sample_rate) % TWO_PI;
        }

        let breath = self.noise.next() * 0.03;

        (harmonics + ensemble * ENSEMBLE_AMP + breath) * CHOIR_NORM
    }
}

// ── Tribal Bass Drum filter ───────────────────────────────────────────────────
//
// Models a large ceremonial drum driven continuously:
//   • Sub-octave (0.5× freq) for deep body resonance
//   • Fundamental + H2 + H3 for the thump "batter head" character
//   • 5 Hz amplitude tremolo — creates the rolling, pulsing tribal feel
//   • 8 % pink noise — skin texture and stick attack character
//   • Heavy normalization keeps it punchy without distorting

/// RMS normalisation for sub + fund + h2 + h3 mix.
/// sqrt(0.55² + 1.0² + 0.60² + 0.25²) ≈ 1.28; tremolo avg 0.85 → /1.10
const DRUM_NORM: f64 = 1.0 / 1.10;

pub struct BassDrumState {
    sub_phase:   f64, // 0.5× freq
    fund_phase:  f64, // fundamental
    h2_phase:    f64, // 2× freq
    h3_phase:    f64, // 3× freq
    trem_phase:  f64, // 5 Hz amplitude tremolo
    noise:       PinkNoiseGen,
}

impl BassDrumState {
    pub fn new() -> Self {
        Self {
            sub_phase:  0.0,
            fund_phase: 0.0,
            h2_phase:   0.0,
            h3_phase:   0.0,
            trem_phase: 0.0,
            noise:      PinkNoiseGen::new(),
        }
    }

    pub fn tick(&mut self, freq: f64, sample_rate: f64) -> f64 {
        const TREM_RATE:  f64 = 5.0;   // Hz — tribal pulse rate
        const TREM_DEPTH: f64 = 0.30;  // ±30 % amplitude swing

        // Amplitude tremolo (the repeating "hit" feel)
        let envelope = 1.0 - TREM_DEPTH + TREM_DEPTH * self.trem_phase.sin().abs();
        self.trem_phase = (self.trem_phase + TWO_PI * TREM_RATE / sample_rate) % TWO_PI;

        // Sub-octave — the deep chest resonance
        let sub = self.sub_phase.sin() * 0.55;
        self.sub_phase = (self.sub_phase + TWO_PI * (freq * 0.5) / sample_rate) % TWO_PI;

        // Fundamental
        let fund = self.fund_phase.sin();
        self.fund_phase = (self.fund_phase + TWO_PI * freq / sample_rate) % TWO_PI;

        // Second harmonic (batter-head boom)
        let h2 = self.h2_phase.sin() * 0.60;
        self.h2_phase = (self.h2_phase + TWO_PI * freq * 2.0 / sample_rate) % TWO_PI;

        // Third harmonic (attack click)
        let h3 = self.h3_phase.sin() * 0.25;
        self.h3_phase = (self.h3_phase + TWO_PI * freq * 3.0 / sample_rate) % TWO_PI;

        // Skin noise
        let skin = self.noise.next() * 0.08;

        (sub + fund + h2 + h3 + skin) * envelope * DRUM_NORM
    }
}

// ── Hebrew Shofar filter ──────────────────────────────────────────────────────
//
// Models a ram's horn (shofar / קרן):
//   • Strong odd harmonics (H3, H5, H7 dominant) — narrow conical bore
//   • Intense 6 Hz vibrato at 0.8 % depth — the player's embouchure wobble
//   • 4 % breath buzz (pink noise) for the rough, organic horn texture
//   • No even-harmonic suppression below the 4th (real shofar has an uneven mix)

/// Harmonic amplitudes H1–H10.  Odd harmonics 3, 5, 7 dominate.
const SHOFAR_HARM_AMPS: [f64; 10] =
    [0.60, 0.50, 0.90, 0.30, 0.80, 0.25, 0.70, 0.20, 0.50, 0.15];

/// RMS normalisation: sqrt(sum(H_i²)) ≈ 1.73
const SHOFAR_NORM: f64 = 1.0 / 1.75;

pub struct ShofarState {
    harm_phases:   [f64; 10],
    vibrato_phase: f64,
    noise:         PinkNoiseGen,
}

impl ShofarState {
    pub fn new() -> Self {
        Self {
            harm_phases:   [0.0; 10],
            vibrato_phase: 0.0,
            noise:         PinkNoiseGen::new(),
        }
    }

    pub fn tick(&mut self, freq: f64, sample_rate: f64) -> f64 {
        const VIBRATO_RATE:  f64 = 6.0;   // Hz — keening wobble of the player
        const VIBRATO_DEPTH: f64 = 0.008; // ±0.8 % — much more expressive than strings
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

// ── Per-oscillator runtime ────────────────────────────────────────────────────
//
// Bundles the base oscillator with all filter states so the engine can route
// through whichever is active without allocating on the hot path.

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

    /// Generate one sample, routing through the active filter.
    pub fn tick(&mut self, waveform: Waveform, filter: Filter, freq: f64, sample_rate: f64) -> f64 {
        // Always advance base phase so the scope stays in sync.
        let base_sample = self.base.tick(waveform, freq, sample_rate);
        match filter {
            Filter::None       => base_sample,
            Filter::Orchestral => self.orchestral.tick(freq, sample_rate),
            Filter::Choir      => self.choir.tick(freq, sample_rate),
            Filter::BassDrum   => self.bass_drum.tick(freq, sample_rate),
            Filter::Shofar     => self.shofar.tick(freq, sample_rate),
        }
    }
}
