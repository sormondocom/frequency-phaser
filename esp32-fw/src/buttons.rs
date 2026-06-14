/// Tactile button handler with software debounce.
///
/// Pin assignment (active-low, internal pull-up enabled):
///   UP     → GPIO 32   next preset in list
///   DOWN   → GPIO 33   previous preset in list
///   LEFT   → GPIO 18   previous category (jumps to its first preset)
///   RIGHT  → GPIO 19   next category
///   SELECT → GPIO 23   toggle play / stop
///
/// Wiring: one leg to GPIO, other leg to GND.  Internal pull-up holds line high
/// at rest; pressing pulls it low.
use esp_idf_hal::gpio::{Gpio18, Gpio19, Gpio23, Gpio32, Gpio33, Input, PinDriver, Pull};

use crate::state::STATE;

const DEBOUNCE_MS: u64 = 30;

pub struct Buttons<'d> {
    up:     PinDriver<'d, Input>,
    down:   PinDriver<'d, Input>,
    left:   PinDriver<'d, Input>,
    right:  PinDriver<'d, Input>,
    select: PinDriver<'d, Input>,

    // Last raw level (true = pressed = low)
    last:   [bool; 5],
    // Timestamp of last confirmed press (ms ticks)
    last_ms: [u64; 5],
}

impl<'d> Buttons<'d> {
    pub fn new(
        up:     Gpio32<'d>,
        down:   Gpio33<'d>,
        left:   Gpio18<'d>,
        right:  Gpio19<'d>,
        select: Gpio23<'d>,
    ) -> anyhow::Result<Self> {
        let up_pin     = PinDriver::input(up,     Pull::Up)?;
        let down_pin   = PinDriver::input(down,   Pull::Up)?;
        let left_pin   = PinDriver::input(left,   Pull::Up)?;
        let right_pin  = PinDriver::input(right,  Pull::Up)?;
        let select_pin = PinDriver::input(select, Pull::Up)?;

        Ok(Self {
            up:      up_pin,
            down:    down_pin,
            left:    left_pin,
            right:   right_pin,
            select:  select_pin,
            last:    [false; 5],
            last_ms: [0; 5],
        })
    }

    /// Call once per UI loop tick (~50 ms).  Fires state changes on debounced
    /// falling edges (release → press).
    pub fn poll(&mut self, now_ms: u64) {
        let raw = [
            !self.up.is_high(),
            !self.down.is_high(),
            !self.left.is_high(),
            !self.right.is_high(),
            !self.select.is_high(),
        ];

        for (i, &pressed) in raw.iter().enumerate() {
            if pressed && !self.last[i] && (now_ms - self.last_ms[i]) >= DEBOUNCE_MS {
                self.last_ms[i] = now_ms;
                on_press(i);
            }
            self.last[i] = pressed;
        }
    }
}

fn on_press(btn: usize) {
    match btn {
        0 => STATE.next_preset(),
        1 => STATE.prev_preset(),
        2 => STATE.prev_category(),
        3 => STATE.next_category(),
        4 => { STATE.toggle_playing(); }
        _ => {}
    }
}
