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
