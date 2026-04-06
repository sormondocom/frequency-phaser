/// Convert a MIDI note number to frequency in Hz.
/// Reference: A4 = MIDI 69 = 440 Hz.
pub fn midi_to_freq(midi: u8) -> f64 {
    440.0 * 2f64.powf((midi as f64 - 69.0) / 12.0)
}

/// Convert a frequency in Hz to the nearest MIDI note number (for display only).
pub fn freq_to_midi(freq: f64) -> u8 {
    let midi = 69.0 + 12.0 * (freq / 440.0).log2();
    midi.round().clamp(21.0, 108.0) as u8
}

const NOTE_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F",
    "F#", "G", "G#", "A", "A#", "B",
];

/// Format a MIDI note number as e.g. "C4", "A#3".
pub fn midi_name(midi: u8) -> String {
    let pc     = (midi % 12) as usize;
    let octave = (midi as i32 / 12) - 1;
    format!("{}{}", NOTE_NAMES[pc], octave)
}

// ── Chord types ───────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ChordType {
    Power,
    Major,
    Minor,
    Diminished,
    Augmented,
    Major7,
    Minor7,
    Dominant7,
    Sus2,
    Sus4,
    Add9,
    Major9,
}

impl ChordType {
    /// Semitone intervals from the root note.
    pub fn intervals(self) -> &'static [i8] {
        match self {
            Self::Power      => &[0, 7],
            Self::Major      => &[0, 4, 7],
            Self::Minor      => &[0, 3, 7],
            Self::Diminished => &[0, 3, 6],
            Self::Augmented  => &[0, 4, 8],
            Self::Major7     => &[0, 4, 7, 11],
            Self::Minor7     => &[0, 3, 7, 10],
            Self::Dominant7  => &[0, 4, 7, 10],
            Self::Sus2       => &[0, 2, 7],
            Self::Sus4       => &[0, 5, 7],
            Self::Add9       => &[0, 4, 7, 14],
            Self::Major9     => &[0, 4, 7, 11, 14],
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Power      => "Power (5)",
            Self::Major      => "Major",
            Self::Minor      => "Minor",
            Self::Diminished => "Diminished",
            Self::Augmented  => "Augmented",
            Self::Major7     => "Major 7th",
            Self::Minor7     => "Minor 7th",
            Self::Dominant7  => "Dominant 7th",
            Self::Sus2       => "Sus2",
            Self::Sus4       => "Sus4",
            Self::Add9       => "Add9",
            Self::Major9     => "Major 9th",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            Self::Power      => "5",
            Self::Major      => "Maj",
            Self::Minor      => "Min",
            Self::Diminished => "Dim",
            Self::Augmented  => "Aug",
            Self::Major7     => "Maj7",
            Self::Minor7     => "Min7",
            Self::Dominant7  => "Dom7",
            Self::Sus2       => "Sus2",
            Self::Sus4       => "Sus4",
            Self::Add9       => "Add9",
            Self::Major9     => "Maj9",
        }
    }

    pub fn all() -> &'static [Self] {
        static ALL: [ChordType; 12] = [
            ChordType::Power,     ChordType::Major,      ChordType::Minor,
            ChordType::Diminished, ChordType::Augmented,
            ChordType::Major7,    ChordType::Minor7,     ChordType::Dominant7,
            ChordType::Sus2,      ChordType::Sus4,
            ChordType::Add9,      ChordType::Major9,
        ];
        &ALL
    }

    pub fn next(self) -> Self {
        let all = Self::all();
        let i = all.iter().position(|&x| x == self).unwrap_or(0);
        all[(i + 1) % all.len()]
    }

    pub fn prev(self) -> Self {
        let all = Self::all();
        let n = all.len();
        let i = all.iter().position(|&x| x == self).unwrap_or(0);
        all[(i + n - 1) % n]
    }
}

// ── Scale types ───────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScaleType {
    Major,
    NaturalMinor,
    HarmonicMinor,
    PentatonicMajor,
    PentatonicMinor,
    Blues,
    Dorian,
    Phrygian,
    Lydian,
    Mixolydian,
    Chromatic,
}

impl ScaleType {
    pub fn intervals(self) -> &'static [i8] {
        match self {
            Self::Major           => &[0, 2, 4, 5, 7, 9, 11],
            Self::NaturalMinor    => &[0, 2, 3, 5, 7, 8, 10],
            Self::HarmonicMinor   => &[0, 2, 3, 5, 7, 8, 11],
            Self::PentatonicMajor => &[0, 2, 4, 7, 9],
            Self::PentatonicMinor => &[0, 3, 5, 7, 10],
            Self::Blues           => &[0, 3, 5, 6, 7, 10],
            Self::Dorian          => &[0, 2, 3, 5, 7, 9, 10],
            Self::Phrygian        => &[0, 1, 3, 5, 7, 8, 10],
            Self::Lydian          => &[0, 2, 4, 6, 7, 9, 11],
            Self::Mixolydian      => &[0, 2, 4, 5, 7, 9, 10],
            Self::Chromatic       => &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Major           => "Major",
            Self::NaturalMinor    => "Natural Minor",
            Self::HarmonicMinor   => "Harmonic Minor",
            Self::PentatonicMajor => "Pentatonic Maj",
            Self::PentatonicMinor => "Pentatonic Min",
            Self::Blues           => "Blues",
            Self::Dorian          => "Dorian",
            Self::Phrygian        => "Phrygian",
            Self::Lydian          => "Lydian",
            Self::Mixolydian      => "Mixolydian",
            Self::Chromatic       => "Chromatic",
        }
    }

    pub fn all() -> &'static [Self] {
        static ALL: [ScaleType; 11] = [
            ScaleType::Major, ScaleType::NaturalMinor, ScaleType::HarmonicMinor,
            ScaleType::PentatonicMajor, ScaleType::PentatonicMinor, ScaleType::Blues,
            ScaleType::Dorian, ScaleType::Phrygian, ScaleType::Lydian,
            ScaleType::Mixolydian, ScaleType::Chromatic,
        ];
        &ALL
    }

    pub fn next(self) -> Self {
        let all = Self::all();
        let i = all.iter().position(|&x| x == self).unwrap_or(0);
        all[(i + 1) % all.len()]
    }

    pub fn prev(self) -> Self {
        let all = Self::all();
        let n = all.len();
        let i = all.iter().position(|&x| x == self).unwrap_or(0);
        all[(i + n - 1) % n]
    }
}

// ── Voicing ───────────────────────────────────────────────────────────────────

/// Controls how chord notes are spread across octaves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Voicing {
    /// All notes within the root octave.
    Close,
    /// Alternate notes raised an octave (widens inner intervals).
    Open,
    /// Each successive note raised an additional octave — orchestral spread.
    Wide,
}

impl Voicing {
    pub fn label(self) -> &'static str {
        match self {
            Self::Close => "Close",
            Self::Open  => "Open",
            Self::Wide  => "Wide",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Close => Self::Open,
            Self::Open  => Self::Wide,
            Self::Wide  => Self::Close,
        }
    }
}

// ── PolyMode (chord vs scale) ─────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PolyMode {
    Chord(ChordType),
    Scale(ScaleType),
}

impl PolyMode {
    pub fn intervals(self) -> &'static [i8] {
        match self {
            Self::Chord(c) => c.intervals(),
            Self::Scale(s) => s.intervals(),
        }
    }

    pub fn label(self) -> String {
        match self {
            Self::Chord(c) => format!("Chord: {}", c.label()),
            Self::Scale(s) => format!("Scale: {}", s.label()),
        }
    }

    pub fn short(self) -> String {
        match self {
            Self::Chord(c) => c.short().to_string(),
            Self::Scale(s) => s.label().to_string(),
        }
    }

    pub fn next_type(self) -> Self {
        match self {
            Self::Chord(c) => Self::Chord(c.next()),
            Self::Scale(s) => Self::Scale(s.next()),
        }
    }

    pub fn prev_type(self) -> Self {
        match self {
            Self::Chord(c) => Self::Chord(c.prev()),
            Self::Scale(s) => Self::Scale(s.prev()),
        }
    }

    pub fn toggle_kind(self) -> Self {
        match self {
            Self::Chord(_) => Self::Scale(ScaleType::Major),
            Self::Scale(_) => Self::Chord(ChordType::Major),
        }
    }

    pub fn is_chord(self) -> bool { matches!(self, Self::Chord(_)) }
}

// ── PolyConfig ────────────────────────────────────────────────────────────────

/// Polyphonic configuration — lives entirely in the UI thread (App).
/// Changes are pushed to oscillator atomics via `apply_poly()`.
#[derive(Clone)]
pub struct PolyConfig {
    pub enabled:    bool,
    /// Root frequency in Hz — exact, no MIDI quantization. Default: C4 = 261.626 Hz.
    pub root_freq:  f64,
    pub mode:       PolyMode,
    pub voicing:    Voicing,
    /// Maximum number of voices to use (capped at MAX_OSCILLATORS and interval count).
    pub max_voices: usize,
}

impl PolyConfig {
    pub fn new() -> Self {
        Self {
            enabled:    false,
            root_freq:  midi_to_freq(60), // C4 ≈ 261.626 Hz
            mode:       PolyMode::Chord(ChordType::Major),
            voicing:    Voicing::Close,
            max_voices: crate::state::MAX_OSCILLATORS,
        }
    }

    /// Display name for the root note (nearest MIDI name, e.g. "C4").
    pub fn root_name(&self) -> String {
        midi_name(freq_to_midi(self.root_freq))
    }

    /// Hz frequencies for all voices in the current configuration.
    pub fn frequencies(&self) -> Vec<f64> {
        let intervals = self.mode.intervals();
        let n         = intervals.len().min(self.max_voices).min(crate::state::MAX_OSCILLATORS);

        intervals[..n]
            .iter()
            .enumerate()
            .map(|(i, &iv)| {
                let extra: i8 = match self.voicing {
                    Voicing::Close => 0,
                    Voicing::Open  => if i % 2 == 1 { 12 } else { 0 },
                    Voicing::Wide  => (i as i8 / 2) * 12,
                };
                self.root_freq * 2f64.powf((iv + extra) as f64 / 12.0)
            })
            .collect()
    }

    /// Human-readable note names (e.g. ["C4", "E4", "G4"]).
    pub fn note_names(&self) -> Vec<String> {
        self.frequencies().iter().map(|&f| midi_name(freq_to_midi(f))).collect()
    }

    /// Shift the root by `semitones` (exact ratio), clamped to a reasonable Hz range.
    pub fn shift_root(&mut self, semitones: i8) {
        let ratio    = 2f64.powf(semitones as f64 / 12.0);
        let new_freq = (self.root_freq * ratio).clamp(
            midi_to_freq(21),  // A0
            midi_to_freq(108), // C8
        );
        self.root_freq = new_freq;
    }
}
