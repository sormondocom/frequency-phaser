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

// ── Per-oscillator runtime ────────────────────────────────────────────────────
//
// Bundles the base oscillator with all filter states so the engine can route
// through whichever is active without allocating on the hot path.
// Adding a future filter (e.g. Choir) means adding a field and a match arm.

pub struct OscillatorRt {
    pub base:       Oscillator,
    pub orchestral: OrchestrialState,
}

impl OscillatorRt {
    pub fn new() -> Self {
        Self {
            base:       Oscillator::new(),
            orchestral: OrchestrialState::new(),
        }
    }

    /// Generate one sample, routing through the active filter.
    pub fn tick(&mut self, waveform: Waveform, filter: Filter, freq: f64, sample_rate: f64) -> f64 {
        match filter {
            Filter::None => self.base.tick(waveform, freq, sample_rate),
            Filter::Orchestral => {
                // Advance base phase too so the scope stays in sync
                self.base.tick(waveform, freq, sample_rate);
                self.orchestral.tick(freq, sample_rate)
            }
        }
    }
}
