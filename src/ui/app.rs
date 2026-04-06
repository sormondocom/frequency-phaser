use std::sync::atomic::Ordering;
use std::sync::Arc;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::music::PolyConfig;
use crate::presets::PRESETS;
use crate::state::{AppState, MAX_FREQ, MIN_FREQ};

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

// ── Input mode ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum InputMode {
    Normal,
    DirectFreq { buffer: String },
    PresetBrowse { selected: usize, scroll: usize },
    /// Poly configuration panel — arrow keys tune root/type, Esc returns to Normal.
    PolyPanel,
}

// ── App ───────────────────────────────────────────────────────────────────────

pub struct App {
    pub state:          Arc<AppState>,
    pub mode:           InputMode,
    pub active_osc:     usize,
    pub step_mode:      StepMode,
    pub status_msg:     Option<String>,
    /// Last preset applied — None until the user explicitly picks one.
    pub current_preset: Option<usize>,
    /// Polyphonic configuration (UI-thread only; pushed to oscillator atomics on change).
    pub poly:           PolyConfig,
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
        }
    }

    /// Handle a crossterm Event. Returns false when the app should quit.
    pub fn handle_event(&mut self, event: Event) -> bool {
        match self.mode.clone() {
            InputMode::Normal                            => self.handle_normal(event),
            InputMode::DirectFreq { buffer }             => self.handle_direct_freq(event, buffer),
            InputMode::PresetBrowse { selected, scroll } => self.handle_preset_browse(event, selected, scroll),
            InputMode::PolyPanel                         => self.handle_poly_panel(event),
        }
    }

    // ── Normal mode ──────────────────────────────────────────────────────────

    fn handle_normal(&mut self, event: Event) -> bool {
        let Event::Key(KeyEvent { code, modifiers, kind, .. }) = event else {
            return true;
        };
        if kind != KeyEventKind::Press {
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

            // Frequency – fine/coarse via shift modifier
            KeyCode::Right => {
                let delta = if modifiers.contains(KeyModifiers::SHIFT) { 0.1 } else { self.step_mode.log_step() };
                self.adjust_freq(delta);
            }
            KeyCode::Left => {
                let delta = if modifiers.contains(KeyModifiers::SHIFT) { 0.1 } else { self.step_mode.log_step() };
                self.adjust_freq(-delta);
            }
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
                    // Re-open the panel for editing
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
                    // Advance to the next preset and apply it immediately
                    let next = self.current_preset
                        .map(|i| (i + 1) % PRESETS.len())
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

            // Direct frequency entry (start with any digit or '.')
            KeyCode::Char(c) if c.is_ascii_digit() || c == '.' => {
                self.mode = InputMode::DirectFreq { buffer: c.to_string() };
            }

            _ => {}
        }
        true
    }

    // ── Direct entry mode ────────────────────────────────────────────────────

    fn handle_direct_freq(&mut self, event: Event, mut buffer: String) -> bool {
        let Event::Key(KeyEvent { code, kind, .. }) = event else {
            return true;
        };
        if kind != KeyEventKind::Press {
            return true;
        }

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
                        self.set_status(format!("→ {}", fmt_freq(freq)));
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
        let total = PRESETS.len();
        let Event::Key(KeyEvent { code, kind, .. }) = event else {
            return true;
        };
        if kind != KeyEventKind::Press {
            return true;
        }

        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                if selected > 0 {
                    selected -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if selected + 1 < total {
                    selected += 1;
                }
            }
            KeyCode::Enter => {
                self.apply_preset(selected);
                self.mode = if self.poly.enabled { InputMode::PolyPanel } else { InputMode::Normal };
                return true;
            }
            KeyCode::Esc | KeyCode::Char('p') | KeyCode::Char('P') => {
                self.mode = if self.poly.enabled { InputMode::PolyPanel } else { InputMode::Normal };
                return true;
            }
            _ => {}
        }

        self.mode = InputMode::PresetBrowse { selected, scroll };
        true
    }

    // ── Poly panel mode ──────────────────────────────────────────────────────

    fn handle_poly_panel(&mut self, event: Event) -> bool {
        let Event::Key(KeyEvent { code, modifiers, kind, .. }) = event else {
            return true;
        };
        if kind != KeyEventKind::Press {
            return true;
        }

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

    // ── Helpers ──────────────────────────────────────────────────────────────

    /// Push the current poly config to oscillator atomics.
    /// Sets osc count to the chord/scale voice count, assigns frequencies,
    /// and enables/disables oscillators accordingly.
    pub fn apply_poly(&mut self) {
        if !self.poly.enabled { return; }

        let freqs = self.poly.frequencies();
        let n     = freqs.len();

        // Bring oscillator count up to n if needed
        let current = self.state.get_osc_count();
        if n > current {
            for _ in current..n {
                self.state.add_oscillator();
            }
        }

        // Set exactly n oscillators for the chord, disable the rest
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
        let p = &PRESETS[idx];
        self.current_preset = Some(idx);

        if self.poly.enabled {
            // Move the entire chord/scale to be rooted at the preset frequency.
            self.poly.root_freq = p.freq;
            self.apply_poly();
        } else {
            self.osc().set_freq(p.freq);
            self.osc().set_waveform(p.waveform);
        }

        self.set_status(format!("[{}/{}] {} — {}", idx + 1, PRESETS.len(), p.name, fmt_freq(p.freq)));
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

    /// Clear the status message (call after it has been displayed long enough).
    pub fn clear_status(&mut self) {
        self.status_msg = None;
    }
}

// ── Frequency formatter (shared with render) ──────────────────────────────────

pub fn fmt_freq(freq: f64) -> String {
    if freq >= 1_000.0 {
        format!("{:.3} kHz", freq / 1_000.0)
    } else if freq >= 1.0 {
        format!("{:.3} Hz", freq)
    } else {
        format!("{:.4} Hz", freq)
    }
}
