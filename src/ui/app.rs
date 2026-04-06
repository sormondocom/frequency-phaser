use std::sync::atomic::Ordering;
use std::sync::Arc;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::music::PolyConfig;
use crate::presets::PRESETS;
use crate::state::{AppState, Waveform, MAX_FREQ, MIN_FREQ};

// ── Step mode ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StepMode {
    Fine,   // log10(freq) ± 0.001  ≈ 0.23 %
    Medium, // log10(freq) ± 0.01   ≈ 2.3 %
    Coarse, // log10(freq) ± 0.1    ≈ 26 %
}

impl StepMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Fine   => "Fine  ",
            Self::Medium => "Medium",
            Self::Coarse => "Coarse",
        }
    }

    pub fn log_step(self) -> f64 {
        match self {
            Self::Fine   => 0.001,
            Self::Medium => 0.01,
            Self::Coarse => 0.1,
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Fine   => Self::Medium,
            Self::Medium => Self::Coarse,
            Self::Coarse => Self::Fine,
        }
    }
}

// ── Custom preset ─────────────────────────────────────────────────────────────

pub struct CustomPreset {
    pub name:     String,
    pub freq:     f64,
    pub waveform: Waveform,
}

// ── Input mode ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum InputMode {
    Normal,
    DirectFreq { buffer: String },
    PresetBrowse { selected: usize, scroll: usize },
    /// Poly configuration panel.
    PolyPanel,
    /// Name-entry dialog for saving a custom preset.
    SavePreset { freq: f64, waveform: Waveform, name_buf: String },
    /// Digit-zone scrubber — cursor selects a place-value column to spin.
    /// Format: `DDDDD.DDD Hz`  columns 0-4 = integer part, 5-7 = fractional.
    DigitTune { cursor: u8 },
}

// ── Digit-zone helpers ────────────────────────────────────────────────────────

/// The 8 digit columns and their Hz place values (index 0 = ten-thousands).
pub const DIGIT_PLACE_VALUES: [f64; 8] = [
    10_000.0, 1_000.0, 100.0, 10.0, 1.0,   // integer part (cols 0-4)
    0.1,      0.01,    0.001,               // fractional part (cols 5-7)
];

/// Format `freq` as an 8-column digit string `"DDDDD.DDD"`.
pub fn fmt_digit_zones(freq: f64) -> String {
    let clamped = freq.clamp(MIN_FREQ, MAX_FREQ);
    let int_part  = clamped as u64;
    let frac_part = ((clamped - int_part as f64) * 1000.0).round() as u64;
    format!("{:05}.{:03}", int_part, frac_part)
}

// ── App ───────────────────────────────────────────────────────────────────────

pub struct App {
    pub state:          Arc<AppState>,
    pub mode:           InputMode,
    pub active_osc:     usize,
    pub step_mode:      StepMode,
    pub status_msg:     Option<String>,
    /// Last preset applied (unified index across custom + built-in).
    pub current_preset: Option<usize>,
    /// Polyphonic configuration.
    pub poly:           PolyConfig,
    /// User-saved custom presets (prepended to the built-in list).
    pub custom_presets: Vec<CustomPreset>,
    /// Consecutive key-repeat count for the held arrow key (for acceleration).
    pub arrow_repeat:   u32,
}

impl App {
    pub fn new(state: Arc<AppState>) -> Self {
        Self {
            state,
            mode:           InputMode::Normal,
            active_osc:     0,
            step_mode:      StepMode::Medium,
            status_msg:     None,
            current_preset: None,
            poly:           PolyConfig::new(),
            custom_presets: Vec::new(),
            arrow_repeat:   0,
        }
    }

    // ── Helpers ──────────────────────────────────────────────────────────────

    /// Total presets = custom (first) + built-in.
    pub fn total_presets(&self) -> usize {
        self.custom_presets.len() + PRESETS.len()
    }

    /// Acceleration multiplier — doubles every 20 key-repeat events, cap 128×.
    fn accel_mult(repeat: u32) -> f64 {
        let x = (repeat as f64 / 20.0).min(7.0);
        2.0f64.powf(x)
    }

    /// Handle a crossterm Event. Returns false when the app should quit.
    pub fn handle_event(&mut self, event: Event) -> bool {
        match self.mode.clone() {
            InputMode::Normal                            => self.handle_normal(event),
            InputMode::DirectFreq { buffer }             => self.handle_direct_freq(event, buffer),
            InputMode::PresetBrowse { selected, scroll } => self.handle_preset_browse(event, selected, scroll),
            InputMode::PolyPanel                         => self.handle_poly_panel(event),
            InputMode::SavePreset { freq, waveform, name_buf } =>
                self.handle_save_preset(event, freq, waveform, name_buf),
            InputMode::DigitTune { cursor }              => self.handle_digit_tune(event, cursor),
        }
    }

    // ── Normal mode ──────────────────────────────────────────────────────────

    fn handle_normal(&mut self, event: Event) -> bool {
        let Event::Key(KeyEvent { code, modifiers, kind, .. }) = event else {
            return true;
        };

        // Arrow keys respond to both Press and Repeat for hold-to-accelerate.
        // Release resets the counter. All other keys: Press only.
        let is_freq_arrow = matches!(code, KeyCode::Left | KeyCode::Right)
            && !modifiers.contains(KeyModifiers::SHIFT);

        if kind == KeyEventKind::Release {
            self.arrow_repeat = 0;
            return true;
        }
        if !is_freq_arrow && kind != KeyEventKind::Press {
            return true;
        }

        match code {
            // Quit
            KeyCode::Char('q') | KeyCode::Char('Q') => return false,

            // Play / stop
            KeyCode::Enter => {
                let next = !self.state.is_playing();
                self.state.playing.store(next, Ordering::Relaxed);
                self.set_status(if next { "Playing" } else { "Stopped" });
            }

            // Frequency — accelerating hold
            KeyCode::Right if !modifiers.contains(KeyModifiers::SHIFT) => {
                if kind == KeyEventKind::Press { self.arrow_repeat = 0; }
                else { self.arrow_repeat = self.arrow_repeat.saturating_add(1); }
                let delta = self.step_mode.log_step() * Self::accel_mult(self.arrow_repeat);
                self.adjust_freq(delta);
            }
            KeyCode::Left if !modifiers.contains(KeyModifiers::SHIFT) => {
                if kind == KeyEventKind::Press { self.arrow_repeat = 0; }
                else { self.arrow_repeat = self.arrow_repeat.saturating_add(1); }
                let delta = self.step_mode.log_step() * Self::accel_mult(self.arrow_repeat);
                self.adjust_freq(-delta);
            }

            // Shift + arrow — single coarse jump
            KeyCode::Right => self.adjust_freq(0.1),
            KeyCode::Left  => self.adjust_freq(-0.1),

            // Decade jumps
            KeyCode::PageUp   => self.adjust_freq(1.0),
            KeyCode::PageDown => self.adjust_freq(-1.0),

            // Volume
            KeyCode::Up => {
                let amp = self.osc().get_amp();
                self.osc().set_amp(amp + 0.05);
            }
            KeyCode::Down => {
                let amp = self.osc().get_amp();
                self.osc().set_amp((amp - 0.05).max(0.0));
            }

            // Master volume
            KeyCode::Char('+') | KeyCode::Char('=') => {
                let v = self.state.get_master_vol();
                self.state.set_master_vol(v + 0.05);
            }
            KeyCode::Char('-') | KeyCode::Char('_') => {
                let v = self.state.get_master_vol();
                self.state.set_master_vol(v - 0.05);
            }

            // Switch oscillator
            KeyCode::Tab => {
                let count = self.state.get_osc_count();
                self.active_osc = (self.active_osc + 1) % count;
            }
            KeyCode::BackTab => {
                let count = self.state.get_osc_count();
                self.active_osc = (self.active_osc + count - 1) % count;
            }

            // Waveform
            KeyCode::Char('w') | KeyCode::Char('W') => {
                let w = self.osc().get_waveform().next();
                self.osc().set_waveform(w);
            }

            // Channel routing
            KeyCode::Char('c') | KeyCode::Char('C') => {
                let c = self.osc().get_channel().next();
                self.osc().set_channel(c);
            }

            // Filter overlay
            KeyCode::Char('f') | KeyCode::Char('F') => {
                let f = self.osc().get_filter().next();
                self.osc().set_filter(f);
                self.set_status(format!("Filter: {}", f.description()));
            }

            // Polyphonic mode
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if self.poly.enabled {
                    self.mode = InputMode::PolyPanel;
                } else {
                    self.poly.enabled = true;
                    self.apply_poly();
                    self.mode = InputMode::PolyPanel;
                    self.set_status(format!(
                        "Poly ON — {} {}",
                        self.poly.root_name(),
                        self.poly.mode.short()
                    ));
                }
            }

            // Step mode
            KeyCode::Char('s') | KeyCode::Char('S') => {
                self.step_mode = self.step_mode.next();
                self.set_status(format!("Step: {}", self.step_mode.label()));
            }

            // Enable / disable oscillator
            KeyCode::Char('e') | KeyCode::Char('E') => {
                let en = self.osc().is_enabled();
                self.osc().set_enabled(!en);
            }

            // Save current frequency as a custom preset
            KeyCode::Char('n') | KeyCode::Char('N') => {
                let freq     = self.osc().get_freq();
                let waveform = self.osc().get_waveform();
                self.mode    = InputMode::SavePreset { freq, waveform, name_buf: String::new() };
            }

            // Add oscillator
            KeyCode::F(1) => {
                if self.state.add_oscillator() {
                    self.active_osc = self.state.get_osc_count() - 1;
                    self.set_status("Oscillator added");
                } else {
                    self.set_status("Maximum oscillators reached");
                }
            }

            // Remove oscillator
            KeyCode::F(2) => {
                let idx = self.active_osc;
                if self.state.remove_oscillator(idx) {
                    let count = self.state.get_osc_count();
                    if self.active_osc >= count {
                        self.active_osc = count - 1;
                    }
                    self.set_status("Oscillator removed");
                }
            }

            // Preset browser / live preset cycle
            KeyCode::Char('p') | KeyCode::Char('P') => {
                if self.state.is_playing() {
                    let next = self.current_preset
                        .map(|i| (i + 1) % self.total_presets())
                        .unwrap_or(0);
                    self.apply_preset(next);
                } else {
                    let sel = self.current_preset.unwrap_or(0);
                    self.mode = InputMode::PresetBrowse {
                        selected: sel,
                        scroll:   sel.saturating_sub(4),
                    };
                }
            }

            // Digit-zone tuner
            KeyCode::Char('/') => {
                self.mode = InputMode::DigitTune { cursor: 4 }; // start at ones column
            }

            // Direct frequency entry
            KeyCode::Char(c) if c.is_ascii_digit() || c == '.' => {
                self.mode = InputMode::DirectFreq { buffer: c.to_string() };
            }

            _ => {}
        }
        true
    }

    // ── Direct entry mode ────────────────────────────────────────────────────

    fn handle_direct_freq(&mut self, event: Event, mut buffer: String) -> bool {
        let Event::Key(KeyEvent { code, kind, .. }) = event else { return true; };
        if kind != KeyEventKind::Press { return true; }

        match code {
            KeyCode::Char(c) if c.is_ascii_digit() || c == '.' => {
                buffer.push(c);
                self.mode = InputMode::DirectFreq { buffer };
            }
            KeyCode::Backspace => {
                buffer.pop();
                if buffer.is_empty() {
                    self.mode = InputMode::Normal;
                } else {
                    self.mode = InputMode::DirectFreq { buffer };
                }
            }
            KeyCode::Enter => {
                match buffer.parse::<f64>() {
                    Ok(freq) if (MIN_FREQ..=MAX_FREQ).contains(&freq) => {
                        self.osc().set_freq(freq);
                        self.set_status(format!("→ {}   [N] to save as preset", fmt_freq(freq)));
                    }
                    Ok(_) => {
                        self.set_status(format!("Out of range ({MIN_FREQ}–{MAX_FREQ} Hz)"));
                    }
                    Err(_) => {
                        self.set_status("Invalid number");
                    }
                }
                self.mode = InputMode::Normal;
            }
            KeyCode::Esc => {
                self.mode = InputMode::Normal;
            }
            _ => {}
        }
        true
    }

    // ── Preset browse mode ───────────────────────────────────────────────────

    fn handle_preset_browse(&mut self, event: Event, mut selected: usize, scroll: usize) -> bool {
        let total = self.total_presets();
        let Event::Key(KeyEvent { code, kind, .. }) = event else { return true; };
        if kind != KeyEventKind::Press { return true; }

        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                if selected > 0 { selected -= 1; }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if selected + 1 < total { selected += 1; }
            }
            KeyCode::Enter => {
                self.apply_preset(selected);
                self.mode = if self.poly.enabled { InputMode::PolyPanel } else { InputMode::Normal };
                return true;
            }
            KeyCode::Char('d') | KeyCode::Char('D') => {
                // Delete a custom preset
                let n_custom = self.custom_presets.len();
                if selected < n_custom {
                    self.custom_presets.remove(selected);
                    let new_total = self.total_presets();
                    selected = selected.min(new_total.saturating_sub(1));
                    // If the active preset was this one or later, shift it
                    if let Some(cur) = self.current_preset {
                        if cur == selected + 1 {
                            self.current_preset = None;
                        } else if cur > selected {
                            self.current_preset = Some(cur - 1);
                        }
                    }
                    self.set_status("Custom preset deleted");
                }
            }
            KeyCode::Esc | KeyCode::Char('p') | KeyCode::Char('P') => {
                self.mode = if self.poly.enabled { InputMode::PolyPanel } else { InputMode::Normal };
                return true;
            }
            _ => {}
        }

        // Auto-scroll: keep selected visible within a window of ~8 rows
        const WIN: usize = 8;
        let new_scroll = if selected < scroll {
            selected
        } else if selected >= scroll + WIN {
            selected + 1 - WIN
        } else {
            scroll
        };

        self.mode = InputMode::PresetBrowse { selected, scroll: new_scroll };
        true
    }

    // ── Poly panel mode ──────────────────────────────────────────────────────

    fn handle_poly_panel(&mut self, event: Event) -> bool {
        let Event::Key(KeyEvent { code, modifiers, kind, .. }) = event else { return true; };
        if kind != KeyEventKind::Press { return true; }

        match code {
            // Play / stop
            KeyCode::Enter => {
                let next = !self.state.is_playing();
                self.state.playing.store(next, Ordering::Relaxed);
                self.set_status(if next { "Playing" } else { "Stopped" });
            }
            // Exit panel — keep poly enabled
            KeyCode::Esc => {
                self.mode = InputMode::Normal;
            }
            // Turn poly off and exit
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                self.poly.enabled = false;
                self.mode = InputMode::Normal;
                self.set_status("Poly OFF");
            }
            // Root note — semitone / octave
            KeyCode::Right => {
                let steps = if modifiers.contains(KeyModifiers::SHIFT) { 12i8 } else { 1 };
                self.poly.shift_root(steps);
                self.apply_poly();
            }
            KeyCode::Left => {
                let steps = if modifiers.contains(KeyModifiers::SHIFT) { 12i8 } else { 1 };
                self.poly.shift_root(-steps);
                self.apply_poly();
            }
            // Chord / scale type
            KeyCode::Down => {
                self.poly.mode = self.poly.mode.next_type();
                self.apply_poly();
            }
            KeyCode::Up => {
                self.poly.mode = self.poly.mode.prev_type();
                self.apply_poly();
            }
            // Voicing
            KeyCode::Char('v') | KeyCode::Char('V') => {
                self.poly.voicing = self.poly.voicing.next();
                self.apply_poly();
            }
            // Toggle Chord ↔ Scale
            KeyCode::Tab => {
                self.poly.mode = self.poly.mode.toggle_kind();
                self.apply_poly();
            }
            // Open preset browser — returns to PolyPanel after selection
            KeyCode::Char('p') | KeyCode::Char('P') => {
                let sel = self.current_preset.unwrap_or(0);
                self.mode = InputMode::PresetBrowse {
                    selected: sel,
                    scroll:   sel.saturating_sub(4),
                };
            }
            _ => {}
        }
        true
    }

    // ── Save preset mode ─────────────────────────────────────────────────────

    fn handle_save_preset(
        &mut self,
        event:    Event,
        freq:     f64,
        waveform: Waveform,
        mut name_buf: String,
    ) -> bool {
        let Event::Key(KeyEvent { code, kind, .. }) = event else { return true; };
        if kind != KeyEventKind::Press { return true; }

        match code {
            KeyCode::Char(c) => {
                name_buf.push(c);
                self.mode = InputMode::SavePreset { freq, waveform, name_buf };
            }
            KeyCode::Backspace => {
                name_buf.pop();
                self.mode = InputMode::SavePreset { freq, waveform, name_buf };
            }
            KeyCode::Enter => {
                let name = if name_buf.trim().is_empty() {
                    fmt_freq(freq)
                } else {
                    name_buf.trim().to_string()
                };
                // Custom presets always land at index 0 (prepended)
                self.custom_presets.insert(0, CustomPreset { name: name.clone(), freq, waveform });
                // Shift current_preset index to account for the new entry at front
                if let Some(ref mut cur) = self.current_preset {
                    *cur += 1;
                }
                self.current_preset = Some(0);
                self.set_status(format!("★ Saved: {} — {}", name, fmt_freq(freq)));
                self.mode = InputMode::Normal;
            }
            KeyCode::Esc => {
                self.mode = InputMode::Normal;
            }
            _ => {}
        }
        true
    }

    // ── Digit-zone tuner ─────────────────────────────────────────────────────

    fn handle_digit_tune(&mut self, event: Event, mut cursor: u8) -> bool {
        let Event::Key(KeyEvent { code, kind, .. }) = event else { return true; };
        if kind != KeyEventKind::Press { return true; }

        let n_cols = DIGIT_PLACE_VALUES.len() as u8; // 8

        match code {
            // Move cursor between digit columns
            KeyCode::Left => {
                if cursor > 0 { cursor -= 1; }
                self.mode = InputMode::DigitTune { cursor };
            }
            KeyCode::Right => {
                if cursor + 1 < n_cols { cursor += 1; }
                self.mode = InputMode::DigitTune { cursor };
            }

            // Spin the active digit up / down
            KeyCode::Up => {
                let place = DIGIT_PLACE_VALUES[cursor as usize];
                let freq  = (self.osc().get_freq() + place).clamp(MIN_FREQ, MAX_FREQ);
                self.osc().set_freq(freq);
                self.mode = InputMode::DigitTune { cursor };
            }
            KeyCode::Down => {
                let place = DIGIT_PLACE_VALUES[cursor as usize];
                let freq  = (self.osc().get_freq() - place).clamp(MIN_FREQ, MAX_FREQ);
                self.osc().set_freq(freq);
                self.mode = InputMode::DigitTune { cursor };
            }

            // Play/stop without leaving
            KeyCode::Enter => {
                let next = !self.state.is_playing();
                self.state.playing.store(next, Ordering::Relaxed);
                self.set_status(if next { "Playing" } else { "Stopped" });
                self.mode = InputMode::DigitTune { cursor };
            }

            // Jump cursor to next larger / smaller decade (PageUp/Down)
            KeyCode::PageUp => {
                if cursor > 0 { cursor -= 1; }
                self.mode = InputMode::DigitTune { cursor };
            }
            KeyCode::PageDown => {
                if cursor + 1 < n_cols { cursor += 1; }
                self.mode = InputMode::DigitTune { cursor };
            }

            // Save from within digit tuner
            KeyCode::Char('n') | KeyCode::Char('N') => {
                let freq     = self.osc().get_freq();
                let waveform = self.osc().get_waveform();
                self.mode    = InputMode::SavePreset { freq, waveform, name_buf: String::new() };
            }

            KeyCode::Esc | KeyCode::Char('/') => {
                self.mode = InputMode::Normal;
            }
            _ => {
                self.mode = InputMode::DigitTune { cursor };
            }
        }
        true
    }

    // ── Shared helpers ────────────────────────────────────────────────────────

    /// Push the current poly config to oscillator atomics.
    pub fn apply_poly(&mut self) {
        if !self.poly.enabled { return; }

        let freqs = self.poly.frequencies();
        let n     = freqs.len();

        let current = self.state.get_osc_count();
        if n > current {
            for _ in current..n {
                self.state.add_oscillator();
            }
        }

        self.state.osc_count.store(n as u32, Ordering::Relaxed);
        for i in 0..crate::state::MAX_OSCILLATORS {
            if i < n {
                self.state.oscillators[i].set_freq(freqs[i]);
                self.state.oscillators[i].set_enabled(true);
            } else {
                self.state.oscillators[i].set_enabled(false);
            }
        }

        if self.active_osc >= n {
            self.active_osc = 0;
        }
    }

    fn apply_preset(&mut self, idx: usize) {
        let n_custom = self.custom_presets.len();
        self.current_preset = Some(idx);

        if idx < n_custom {
            let cp   = &self.custom_presets[idx];
            let freq = cp.freq;
            let name = cp.name.clone();
            if self.poly.enabled {
                self.poly.root_freq = freq;
                self.apply_poly();
            } else {
                self.osc().set_freq(freq);
                self.osc().set_waveform(cp.waveform);
            }
            self.set_status(format!("[★] {} — {}", name, fmt_freq(freq)));
        } else {
            let i = idx - n_custom;
            let p = &PRESETS[i];
            if self.poly.enabled {
                self.poly.root_freq = p.freq;
                self.apply_poly();
            } else {
                self.osc().set_freq(p.freq);
                self.osc().set_waveform(p.waveform);
            }
            self.set_status(format!(
                "[{}/{}] {} — {}",
                i + 1, PRESETS.len(), p.name, fmt_freq(p.freq)
            ));
        }
    }

    fn osc(&self) -> &crate::state::OscillatorState {
        &self.state.oscillators[self.active_osc]
    }

    fn adjust_freq(&self, log_delta: f64) {
        let current  = self.osc().get_freq();
        let log_freq = current.log10() + log_delta;
        let new_freq = 10f64.powf(log_freq).clamp(MIN_FREQ, MAX_FREQ);
        self.osc().set_freq(new_freq);
    }

    fn set_status(&mut self, msg: impl Into<String>) {
        self.status_msg = Some(msg.into());
    }

    pub fn clear_status(&mut self) {
        self.status_msg = None;
    }
}

// ── Frequency formatter ───────────────────────────────────────────────────────

pub fn fmt_freq(freq: f64) -> String {
    if freq >= 1_000.0 {
        format!("{:.3} kHz", freq / 1_000.0)
    } else if freq >= 1.0 {
        format!("{:.3} Hz", freq)
    } else {
        format!("{:.4} Hz", freq)
    }
}
