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
| `U` | Upload MP3/WAV to ESP32 via serial (UART transfer) |
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

The same oscillator core runs on an **ESP-WROOM-32**, producing audio via I2S and presenting a 7-button UI on an OLED display — no computer required.

### Hardware

| Component | Part | Notes |
|-----------|------|-------|
| MCU | ESP-WROOM-32 | Any ESP32 DevKit works |
| DAC | PCM5102A | I2S; FMT → GND, XSMT → 3.3V, SCK → GND |
| Display | SSD1306 128×64 OLED | I2C at 0x3C; VCC must be 5V |
| Navigation | 7 × tactile buttons | Active-low, internal pull-ups enabled |

### Wiring

```
PCM5102A DAC (I2S audio)
  BCK  → GPIO 27    LRCK → GPIO 26    DIN  → GPIO 25
  FMT  → GND        XSMT → 3.3V       SCK  → GND

SSD1306 OLED (I2C)                    ← VCC must be 5V, not 3.3V
  SDA  → GPIO 21    SCL  → GPIO 22

Buttons (other leg → GND, internal pull-ups enabled)
  UP     → GPIO 32    DOWN   → GPIO 33
  LEFT   → GPIO 18    RIGHT  → GPIO 19
  SELECT → GPIO 23
  VOL+   → GPIO 4     VOL-   → GPIO 5
```

### OLED Display

The display has two layouts depending on mode:

**Normal mode** — preset browser

```
┌─────────────────────────────┐  ← yellow strip
│ 7.83 Hz               STOP  │
├─────────────────────────────┤
│ Schumann                    │  category
│ Schumann 7.83 Hz            │  preset name
│ SINE RAW [1/67]             │  waveform · filter · index
│ U/D:prst  L/R:+/-Hz         │  hint
└─────────────────────────────┘
```

**Tuning mode** — entered by pressing LEFT or RIGHT

```
┌─────────────────────────────┐  ← yellow strip (frequency updates live)
│ 8.50 Hz               PLAY  │
├─────────────────────────────┤
│  ∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿  │
│ ∿                         ∿ │  waveform graphic (one cycle, full width)
│   ∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿∿   │
│ SINE / RAW                  │  waveform · filter
│ L/R:Hz   U/D:preset         │  hint (shows L/R:1Hz in fine step mode)
└─────────────────────────────┘
```

### Controls

**Always available**

| Button | Action |
|--------|--------|
| SELECT | Toggle play / stop |
| VOL+   | Volume up 5% (max 100%) |
| VOL-   | Volume down 5% (min 0%) |

**Normal mode** — preset browser

| Button | Action |
|--------|--------|
| UP     | Next preset |
| DOWN   | Previous preset |
| LEFT / RIGHT | Enter tuning mode (frequency down / up) |

**Tuning mode** — entered on the first LEFT or RIGHT press

The frequency starts at the current preset's value. UP or DOWN exits tuning and returns to preset browsing.

| Button | Action |
|--------|--------|
| LEFT  | Frequency down |
| RIGHT | Frequency up   |
| UP    | Previous preset (exits tuning) |
| DOWN  | Next preset (exits tuning) |

**Step size** scales automatically with the current frequency:

| Range | Step |
|-------|------|
| < 1 Hz | 0.01 Hz |
| 1 – 10 Hz | 0.1 Hz |
| 10 – 100 Hz | 1 Hz |
| 100 Hz – 1 kHz | 5 Hz |
| > 1 kHz | 50 Hz |

**Hold-to-repeat** — holding LEFT or RIGHT repeats after 400 ms and accelerates:

| Hold duration | Rate |
|---------------|------|
| 0.4 – 1 s | 20 steps / sec |
| 1 – 2 s   | 40 steps / sec |
| 2 s+      | 80 steps / sec |

**MP3 file browser** — opened with SELECT + DOWN simultaneously

```
┌─────────────────────────────┐  ← yellow strip
│ MP3 Files             STOP  │
├─────────────────────────────┤
│ song.mp3                    │  ← selected file (inverted highlight)
│ clip.wav                    │
│                             │
│ SEL:play  SEL+U:back        │  hint
└─────────────────────────────┘
```

| Gesture | Action |
|---------|--------|
| SELECT + DOWN (simultaneous) | Open MP3 browser |
| SELECT + UP   (simultaneous) | Return to normal mode |
| UP / DOWN | Navigate file list |
| SELECT | Play / stop (oscillator tone) |

Files must be uploaded first via the desktop app (`U` key). The SPIFFS partition holds ~2.75 MB.

> **Note:** MP3/WAV audio *decoding* on the ESP32 is not yet implemented — the file browser shows uploaded files and SELECT toggles the oscillator tone as usual. Full audio playback is a planned future feature requiring an MP3 decoder (minimp3/HELIX or similar).

**USB transfer screen** — shown automatically while a file transfer is in progress

```
┌─────────────────────────────┐  ← yellow strip
│ USB TRANSFER                │
├─────────────────────────────┤
│ Receiving file...           │
│ [████████████░░░░░░░░░░░░]  │  progress bar
│  67%  68432/102400 B        │
│ Do not disconnect           │
└─────────────────────────────┘
```

**Two-button gestures** (LEFT + RIGHT together):

| Gesture | Action |
|---------|--------|
| Both pressed within 100 ms of each other | Round frequency to nearest whole Hz |
| Hold one, then press the other (> 100 ms apart) | Toggle **fine step mode** — step locked to 1 Hz regardless of frequency. Display hint changes to `L/R:1Hz`. Same gesture toggles it back off. |

### Uploading Audio Files

You can transfer MP3 or WAV files from the desktop app to the ESP32's internal flash (SPIFFS filesystem) over the same USB cable used for programming.

**Capacity:** The custom partition table allocates **2.75 MB** to SPIFFS, enough for one or two short audio clips (e.g. a 1-minute 128 kbps MP3 ≈ 960 KB).

**Transfer rate:** ~11 KB/s at 115 200 baud — 1 MB takes about 90 seconds.

**Workflow:**

1. Flash the firmware as normal: `cargo run --release` (opens the serial monitor)
2. **Close the serial monitor** — the port must not be held open by any other program
3. In the **desktop app**, press **`U`** to open the upload screen
4. Select the ESP32 serial port (e.g. `COM5`) with `↑ ↓`, then `Enter`
5. Type the full path to your MP3 or WAV file, then `Enter`
6. The desktop shows a progress bar; the **ESP32 OLED switches to the USB transfer screen** automatically — if it does not, the handshake failed (see Troubleshooting)
7. Wait for the progress bar to reach 100% — the ESP32 OLED returns to normal mode

Once uploaded, the file appears in the **MP3 file browser** (SELECT + DOWN on the ESP32).

> **Note:** A microSD SPI adapter can replace SPIFFS for larger storage. Wire MOSI → GPIO 13, CLK → GPIO 14, CS → GPIO 15, MISO → GPIO 34 (input-only pin). Avoid GPIO 12 (bootstrap).

> **Transfer rate:** ~11 KB/s at 115 200 baud — 1 MB ≈ 90 seconds.

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
    serial_upload.rs    # UART file transfer to ESP32 (FPUPLOAD protocol)
    state.rs            # Lock-free shared state (AtomicU64/U32/Bool)
    music.rs            # Chord/scale theory, PolyConfig, MIDI ↔ Hz helpers
    presets.rs          # 67+ frequency presets
    ui/
      app.rs            # Event handling, InputMode state machine
      render.rs         # ratatui layout, dial, poly panel, preset sidebar

esp32-fw/
  src/
    audio.rs            # I2S driver task (PCM5102A via I2S0)
    buttons.rs          # 7 tactile buttons — debounce, hold-repeat, gesture detection
    display.rs          # SSD1306 renderer via I2C
    state.rs            # Atomic shared state (same pattern as desktop)
    storage.rs          # SPIFFS filesystem mount + path helper
    upload.rs           # UART upload receiver (FPUPLOAD protocol)
    main.rs             # Boot, thread spawn, 20 Hz UI loop
  partitions.csv        # Custom partition table — factory 1.25 MB + SPIFFS 2.75 MB
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

### SPIFFS: "spiffs partition could not be found"

The device was flashed before the custom partition table was added, so the old single-app partition layout (no SPIFFS entry) is still on flash.

**Fix:** Ensure the runner in `esp32-fw/.cargo/config.toml` includes an explicit `--partition-table` flag pointing to the project CSV with an **absolute path**:

```toml
runner = "espflash flash --chip esp32 --baud 115200 --partition-table K:/fp/esp32-fw/partitions.csv --monitor"
```

A relative path will not work — espflash resolves it from wherever `cargo run` is invoked, not from the package root. On the next `cargo run --release` espflash will flash the correct partition table (factory 1.25 MB + SPIFFS 2.75 MB) at offset `0x8000`.

**First boot after reflash:** SPIFFS will log `mount failed, -10025. formatting...` — this is expected. The filesystem is being initialised on blank flash. Subsequent boots mount silently.

> **espflash checksum behaviour:** If the partition table on flash already matches the CSV, espflash skips that region and shows no upload bar for it. This is normal — it means the table is already correct.

### Upload timeout / ESP32 OLED does not show transfer screen

The desktop app shows a timeout and the ESP32 never enters USB transfer mode. Most likely cause: opening the serial port asserts DTR, which triggers the ESP32 auto-reset circuit. The device reboots and the handshake header sent immediately after is lost.

The desktop app already works around this (it deasserts DTR/RTS and waits 1.5 seconds before sending), but if you still see timeouts:

- Make sure **no serial monitor** is open on the same port (espflash `--monitor`, VS Code serial monitor, PlatformIO, etc.)
- Try disconnecting and reconnecting the USB cable, then retry the upload immediately
- On some USB-serial chips (CP2102, CH340) the reset circuit is always wired; the 1.5 s delay should be sufficient

### Upload fails with `uart driver error` loop on ESP32

The serial monitor shows a rapid stream of `E uart: uart_read_bytes(1504): uart driver error`. This means `uart_driver_install()` was not called before `uart_read_bytes()` — the interrupt-driven RX ring buffer does not exist.

This is handled automatically in `upload::run_listener()` — if you see this error after rebuilding, ensure you have the latest firmware flashed (`cargo run --release`).

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
