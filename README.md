# Frequency Phaser

<p align="center">
  <img src="mascot.svg" alt="The Phaser — mascot" width="300"/>
</p>

<p align="center">
  <em>A multi-oscillator frequency generator — available as a cross-platform terminal app and as standalone ESP32 hardware.</em>
</p>

<p align="center">
  <a href="http://buymeacoffee.com/sormondocom">
    <img src="https://img.shields.io/badge/Buy%20Me%20a%20Coffee-support%20this%20project-FFDD00?style=for-the-badge&logo=buy-me-a-coffee&logoColor=black" alt="Buy Me a Coffee"/>
  </a>
</p>

---

> *"In theory, if you load a Taylor Swift MP3 and tune the resonance dial to the Schumann frequency,
> you could drill through stone. We accept no liability for geological incidents."*

---

## Project Structure

```
fp/
├── core/           # Shared oscillator engine (no_std compatible)
├── desktop/        # Terminal TUI app (Windows / macOS / Linux)
└── esp32-fw/       # ESP-WROOM-32 standalone hardware firmware
```

`core` is a library shared by both targets — the same oscillator math runs on your laptop and on the microcontroller.

---

## Desktop App

A full-featured terminal UI built with [ratatui](https://crates.io/crates/ratatui). Runs anywhere Rust does.

### Capabilities

**Multi-Oscillator Engine**
- Up to **8 simultaneous oscillators**, each independently tunable
- Per-oscillator **frequency** (0.01 Hz – 96 kHz), **amplitude**, **waveform**, and **stereo channel routing** (L / R / L+R)
- Lock-free audio thread — no mutexes on the hot path
- Audio is non-fatal — the UI works without a sound card (useful for frequency planning)

**Waveforms**
| Symbol | Name | Description |
|--------|------|-------------|
| `∿` | Sine | Pure tone |
| `⊓` | Square | Odd harmonics, hollow |
| `⋀` | Triangle | Softer odd harmonics |
| `⟋` | Sawtooth | Full harmonic series, bright |
| `≋` | Pink Noise | 1/f broadband noise |

**Filters**
| Filter | Description |
|--------|-------------|
| `None` | Raw waveform |
| `Orchestral` | Additive harmonics + ensemble detuning + 5.5 Hz vibrato + bow noise |

**Preset Library (67+ presets)**
| Category | Examples |
|----------|---------|
| **Schumann** | 7.83 Hz, 14.3 Hz, 20.8 Hz, 27.3 Hz, 33.8 Hz |
| **Brainwave** | Delta (0.5–4 Hz), Theta, Alpha, Beta, Gamma |
| **Solfeggio** | 174, 285, 396, 417, 528, 639, 741, 852, 963 Hz |
| **Chakra (Traditional)** | Root 256 Hz → Crown 963 Hz |
| **Chakra (Vedic)** | Root 194.18 Hz → Crown 172.06 Hz |
| **Musical** | A432, A444, Concert A440, C256, Middle C |
| **Healing** | Tibetan 432, Earth Resonance, Golden Ratio |
| **Geotechnical** | Seismic P-wave, S-wave, Rayleigh, Love wave, micro-tremor, soil resonance |
| **Reference** | 1 Hz, 10 Hz, 100 Hz, 1 kHz, 10 kHz, 20 kHz, sub-bass, infrasound |

**Polyphonic Mode**
Layer up to 8 voices as a chord or scale rooted at any frequency:
- **12 chord types**: Power, Major, Minor, Diminished, Augmented, Major 7th, Minor 7th, Dominant 7th, Sus2, Sus4, Add9, Major 9th
- **11 scale types**: Major, Natural Minor, Harmonic Minor, Pentatonic Maj/Min, Blues, Dorian, Phrygian, Lydian, Mixolydian, Chromatic
- **3 voicings**: Close · Open · Wide

### Controls

**Normal Mode**

| Key | Action |
|-----|--------|
| `Enter` | Play / Stop |
| `← →` | Adjust frequency (current step size) |
| `Shift + ← →` | Coarse frequency adjust |
| `Page Up / Down` | Decade jump (×10 / ÷10) |
| `↑ ↓` | Oscillator volume ±5% |
| `+ / -` | Master volume ±5% |
| `Tab / Shift+Tab` | Cycle active oscillator |
| `W` | Next waveform |
| `F` | Cycle filter |
| `L` | Load WAV/MP3 as custom file filter |
| `S` | Cycle step mode (Fine / Medium / Coarse) |
| `E` | Enable / disable active oscillator |
| `Y` | Enable polyphonic mode + open Poly Panel |
| `P` | Preset browser (stopped) / cycle presets (playing) |
| `F1` | Add oscillator |
| `F2` | Remove active oscillator |
| `0–9 .` | Begin direct frequency entry |
| `Q` | Quit |

**Poly Panel (`Y`)**

| Key | Action |
|-----|--------|
| `Enter` | Play / Stop |
| `← →` | Shift root note by semitone |
| `Shift + ← →` | Shift root note by octave |
| `↑ ↓` | Previous / next chord or scale type |
| `Tab` | Toggle Chord ↔ Scale mode |
| `V` | Cycle voicing (Close → Open → Wide) |
| `Y` | Turn polyphony off |

### Building the Desktop App

```bash
# Prerequisites: Rust stable (1.70+), working audio output device

# From the workspace root:
cargo build --release
cargo run --release

# Or from the desktop/ subdirectory:
cd desktop
cargo run --release
```

**Runtime dependencies:** [`cpal`](https://crates.io/crates/cpal) (WASAPI / CoreAudio / ALSA), [`ratatui`](https://crates.io/crates/ratatui), [`crossterm`](https://crates.io/crates/crossterm), [`symphonia`](https://crates.io/crates/symphonia) (WAV/MP3 file playback)

---

## ESP32 Hardware

The same oscillator core runs on an **ESP-WROOM-32**, producing audio via I2S and presenting a 5-button navigation UI on an OLED display — no computer required.

### Hardware

| Component | Part | Notes |
|-----------|------|-------|
| MCU | ESP-WROOM-32 | Any ESP32 DevKit works |
| DAC | PCM5102A | I2S; FMT → GND, XSMT → 3.3V, SCK → GND |
| Display | SSD1306 128×64 OLED | I2C at 0x3C |
| Buttons | 5× tactile (active-low) | Internal pull-ups enabled |

### Wiring

```
PCM5102A DAC (I2S audio)
  BCK  → GPIO 27    LRCK → GPIO 26    DIN  → GPIO 25

SSD1306 OLED (I2C)
  SDA  → GPIO 21    SCL  → GPIO 22

Buttons (other leg → GND)
  UP     → GPIO 32    DOWN   → GPIO 33
  LEFT   → GPIO 18    RIGHT  → GPIO 19
  SELECT → GPIO 23
```

### Building the Firmware

The firmware requires the Espressif Rust toolchain. Install it once with [espup](https://github.com/esp-rs/espup):

```powershell
cargo install espup
espup install
```

Then build and flash:

```powershell
cd esp32-fw
cargo build --release          # build only
cargo run --release            # build + flash + open serial monitor
```

**Flashing note:** If `espflash` fails to connect, hold the **BOOT** button on the ESP32, run `cargo run --release`, and release BOOT once `Connecting...` appears. This manually triggers download mode.

**First run** will download ESP-IDF 5.2 (~600 MB) — subsequent builds are fast.

---

## Architecture

```
core/                   # no_std oscillator library
  src/
    lib.rs              # OscillatorRt, waveforms, filters, presets

desktop/
  src/
    audio/
      engine.rs         # cpal stream setup, audio callback
      generator.rs      # Oscillator, OrchestrialState, PinkNoiseGen
    state.rs            # Lock-free shared state (AtomicU64/U32/Bool)
    music.rs            # Chord/scale theory, PolyConfig, MIDI ↔ Hz helpers
    presets.rs          # 67+ frequency presets
    ui/
      app.rs            # Event handling, InputMode state machine
      render.rs         # ratatui layout, dial, poly panel, preset sidebar

esp32-fw/
  src/
    audio.rs            # I2S driver task (PCM5102A via I2S0)
    buttons.rs          # GPIO input with software debounce
    display.rs          # SSD1306 renderer via I2C
    state.rs            # Atomic shared state (same pattern as desktop)
    main.rs             # Boot, thread spawn, 20 Hz UI loop
  build.rs              # embuild ESP-IDF sysenv export
```

---

## Troubleshooting

### Windows: build fails with "Too long output directory"

The `esp-idf-sys` build script enforces an 88-character limit on the Cargo `OUT_DIR` path. On Windows the path accumulates quickly.

**Fix:** Keep the project at a short root path (e.g. `K:\fp`) and redirect the firmware's target directory in `esp32-fw/.cargo/config.toml`:

```toml
[build]
target-dir = "K:\\fp\\esp32-fw\\t"
```

Also move `CARGO_HOME` and `RUSTUP_HOME` to short paths (e.g. `K:\rust` and `K:\rustup`) and update your `PATH` accordingly. Windows `subst` does **not** work as a workaround — the build script resolves real paths.

### esp-idf-sys / esp-idf-hal version compatibility

The crates `esp-idf-sys`, `esp-idf-hal`, and `esp-idf-svc` must be kept in sync with each other and with the target ESP-IDF version. The versions used in this project:

| Crate | Version | ESP-IDF |
|-------|---------|---------|
| `esp-idf-sys` | 0.37 | 5.2.x |
| `esp-idf-hal` | 0.46 | 5.2.x |
| `esp-idf-svc` | 0.52 | 5.2.x |

Set the ESP-IDF version explicitly in `esp32-fw/.cargo/config.toml` — without it the build script defaults to v4.4.6 which is incompatible with the current Rust xtensa toolchain:

```toml
[env]
ESP_IDF_VERSION = "v5.2.5"
```

The `espidf_time64` rustflag is also required; ESP-IDF 5.x uses 64-bit `time_t` and the Rust xtensa standard library expects it:

```toml
[target.xtensa-esp32-espidf]
rustflags = ["--cfg", "espidf_time64"]
```

### esp-idf-hal 0.43 → 0.46 API changes

If upgrading from older esp-idf-hal:

- `esp_idf_hal::peripheral::Peripheral` is gone — use `InputPin`, `OutputPin`, `I2c`, `I2s` trait bounds directly
- `PinDriver<'d, PIN, MODE>` → `PinDriver<'d, MODE>` (pin type param removed)
- `PinDriver::input(pin)` + `.set_pull(Pull::Up)` → single call: `PinDriver::input(pin, Pull::Up)`
- `I2sDriver::new_std_tx` argument order changed — `ws` (word select) moved to last position
- `StdConfig::new(Config, SlotConfig)` → `StdConfig::philips(sample_rate_hz, DataBitWidth)`
- `esp-idf-svc::log` requires `features = ["alloc"]`
- `embuild::espidf` requires `features = ["espidf"]`

---

## License

GNU General Public License v3.0 — see [LICENSE](LICENSE) for the full text.
