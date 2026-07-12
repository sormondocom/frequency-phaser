/// Tactile button handler with software debounce, hold-to-repeat, and gesture detection.
///
/// Pin assignment (active-low, internal pull-up enabled):
///   UP     → GPIO 32   next preset / mp3-prev  (exits tuning mode)
///   DOWN   → GPIO 33   prev preset / mp3-next  (exits tuning mode)
///   LEFT   → GPIO 18   frequency down (enters tuning mode on first press)
///   RIGHT  → GPIO 19   frequency up   (enters tuning mode on first press)
///   SELECT → GPIO 23   toggle play / stop
///   VOL+   → GPIO 4    volume up
///   VOL-   → GPIO 5    volume down
///
/// Wiring: one leg to GPIO, other leg to GND.
///
/// Simultaneous gestures (buttons pressed within SIMULTANEOUS_MS of each other):
///   LEFT  + RIGHT  → round frequency to nearest whole Hz          (tuning mode)
///   LEFT  + RIGHT  → toggle fine-step mode                        (hold one, then other)
///   SELECT + DOWN  → enter MP3 file browser                       (normal mode)
///   SELECT + UP    → exit  MP3 file browser → normal mode
///
/// In MP3 mode UP/DOWN navigate the file list; SELECT still toggles play/stop.
use esp_idf_hal::gpio::{Gpio4, Gpio5, Gpio18, Gpio19, Gpio23, Gpio32, Gpio33, Input, PinDriver, Pull};

use crate::state::STATE;
use crate::storage;

const DEBOUNCE_MS:      u64 = 30;
const HOLD_INITIAL_MS:  u64 = 400;
const HOLD_REPEAT_MS:   u64 = 50;
const SIMULTANEOUS_MS:  u64 = 100;

pub struct Buttons<'d> {
    up:       PinDriver<'d, Input>,
    down:     PinDriver<'d, Input>,
    left:     PinDriver<'d, Input>,
    right:    PinDriver<'d, Input>,
    select:   PinDriver<'d, Input>,
    vol_up:   PinDriver<'d, Input>,
    vol_dn:   PinDriver<'d, Input>,
    last:      [bool; 7],
    press_ms:  [u64; 7], // when button was last pressed (falling edge)
    last_ms:   [u64; 7], // when button last fired an action
    both_last: bool,     // L+R both pressed last tick
    sel_up_last: bool,   // SELECT+UP both pressed last tick
    sel_dn_last: bool,   // SELECT+DOWN both pressed last tick
}

impl<'d> Buttons<'d> {
    pub fn new(
        up:     Gpio32<'d>,
        down:   Gpio33<'d>,
        left:   Gpio18<'d>,
        right:  Gpio19<'d>,
        select: Gpio23<'d>,
        vol_up: Gpio4<'d>,
        vol_dn: Gpio5<'d>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            up:     PinDriver::input(up,     Pull::Up)?,
            down:   PinDriver::input(down,   Pull::Up)?,
            left:   PinDriver::input(left,   Pull::Up)?,
            right:  PinDriver::input(right,  Pull::Up)?,
            select: PinDriver::input(select, Pull::Up)?,
            vol_up: PinDriver::input(vol_up, Pull::Up)?,
            vol_dn: PinDriver::input(vol_dn, Pull::Up)?,
            last:        [false; 7],
            press_ms:    [0; 7],
            last_ms:     [0; 7],
            both_last:   false,
            sel_up_last: false,
            sel_dn_last: false,
        })
    }

    /// Raw instantaneous reading — true when the button is physically held down.
    /// Used by the format confirmation flow before the normal poll loop starts.
    pub fn select_held(&self) -> bool { !self.select.is_high() }
    pub fn left_held(&self)   -> bool { !self.left.is_high() }

    /// Call once per UI tick (~50 ms).
    pub fn poll(&mut self, now_ms: u64) {
        let raw = [
            !self.up.is_high(),     // 0: UP
            !self.down.is_high(),   // 1: DOWN
            !self.left.is_high(),   // 2: LEFT
            !self.right.is_high(),  // 3: RIGHT
            !self.select.is_high(), // 4: SELECT
            !self.vol_up.is_high(), // 5: VOL+
            !self.vol_dn.is_high(), // 6: VOL-
        ];

        // ── Pre-pass: stamp all falling edges ────────────────────────────────
        for (i, &pressed) in raw.iter().enumerate() {
            if pressed && !self.last[i] {
                self.press_ms[i] = now_ms;
            }
        }

        // ── SELECT + DOWN simultaneously → enter MP3 mode ────────────────────
        let sel_dn = raw[4] && raw[1];
        if sel_dn && !self.sel_dn_last {
            let gap = self.press_ms[4].abs_diff(self.press_ms[1]);
            if gap <= SIMULTANEOUS_MS && !STATE.is_mp3_mode() {
                storage::refresh_audio_files();
                STATE.enter_mp3_mode();
            }
        }
        if !sel_dn && self.sel_dn_last {
            // Burst prevention: don't fire SELECT or DOWN individually right after.
            self.last_ms[4] = now_ms;
            self.last_ms[1] = now_ms;
        }
        self.sel_dn_last = sel_dn;

        // ── SELECT + UP simultaneously → exit MP3 mode ───────────────────────
        let sel_up = raw[4] && raw[0];
        if sel_up && !self.sel_up_last {
            let gap = self.press_ms[4].abs_diff(self.press_ms[0]);
            if gap <= SIMULTANEOUS_MS && STATE.is_mp3_mode() {
                STATE.exit_mp3_mode();
            }
        }
        if !sel_up && self.sel_up_last {
            self.last_ms[4] = now_ms;
            self.last_ms[0] = now_ms;
        }
        self.sel_up_last = sel_up;

        // ── LEFT + RIGHT simultaneously (tuning mode gestures) ────────────────
        let both = raw[2] && raw[3];
        if both && !self.both_last {
            let gap = self.press_ms[2].abs_diff(self.press_ms[3]);
            if gap <= SIMULTANEOUS_MS {
                STATE.round_freq();
            } else {
                STATE.toggle_fine_step();
            }
        }
        if !both && self.both_last {
            self.last_ms[2] = now_ms;
            self.last_ms[3] = now_ms;
        }
        self.both_last = both;

        // ── Per-button processing ─────────────────────────────────────────────
        for (i, &pressed) in raw.iter().enumerate() {
            // Suppress buttons that are part of an active simultaneous pair.
            if sel_dn && (i == 4 || i == 1) { self.last[i] = pressed; continue; }
            if sel_up && (i == 4 || i == 0) { self.last[i] = pressed; continue; }
            if both   && (i == 2 || i == 3) { self.last[i] = pressed; continue; }

            if pressed {
                if !self.last[i] {
                    // Falling edge — fire once after debounce.
                    if now_ms.saturating_sub(self.last_ms[i]) >= DEBOUNCE_MS {
                        self.last_ms[i] = now_ms;
                        on_press(i);
                    }
                } else if i == 2 || i == 3 {
                    // Hold-to-repeat with acceleration for LEFT and RIGHT only.
                    let held = now_ms.saturating_sub(self.press_ms[i]);
                    if held >= HOLD_INITIAL_MS
                        && now_ms.saturating_sub(self.last_ms[i]) >= HOLD_REPEAT_MS
                    {
                        self.last_ms[i] = now_ms;
                        let steps = if held > 2_000 { 4 }
                                    else if held > 1_000 { 2 }
                                    else { 1 };
                        for _ in 0..steps { on_press(i); }
                    }
                }
            }

            self.last[i] = pressed;
        }
    }
}

fn on_press(btn: usize) {
    let mp3 = STATE.is_mp3_mode();
    match btn {
        0 => { // UP
            if mp3 { STATE.mp3_prev(storage::audio_file_count()); }
            else   { STATE.next_preset(); }
        }
        1 => { // DOWN
            if mp3 { STATE.mp3_next(storage::audio_file_count()); }
            else   { STATE.prev_preset(); }
        }
        2 => STATE.freq_step(false), // LEFT  → frequency down
        3 => STATE.freq_step(true),  // RIGHT → frequency up
        4 => { STATE.toggle_playing(); }
        5 => STATE.vol_up(),
        6 => STATE.vol_down(),
        _ => {}
    }
}
