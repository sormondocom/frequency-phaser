/// Frequency Phaser — ESP32 firmware
///
/// Wiring summary
/// ─────────────────────────────────────────────────────────────
/// PCM5102A DAC (I2S audio out)
///   BCK  → GPIO 27    LRCK → GPIO 26    DIN  → GPIO 25
///   FMT  → GND        XSMT → 3V3        SCK  → GND
///
/// SSD1306 OLED 128×64 (I2C)
///   SDA  → GPIO 21    SCL  → GPIO 22
///
/// Tactile buttons (active-low, internal pull-up)
///   UP     → GPIO 32  (next preset)
///   DOWN   → GPIO 33  (prev preset)
///   LEFT   → GPIO 18  (prev category)
///   RIGHT  → GPIO 19  (next category)
///   SELECT → GPIO 23  (play / stop)
/// ─────────────────────────────────────────────────────────────
use anyhow::Result;
use esp_idf_hal::peripherals::Peripherals;
use std::{thread, time::Duration};

mod audio;
mod buttons;
mod display;
mod state;

fn main() -> Result<()> {
    // Required esp-idf glue — must be the very first call.
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    log::info!("Frequency Phaser ESP32 starting");

    let peripherals = Peripherals::take()?;

    // ── Audio thread ──────────────────────────────────────────────────────────
    // Spawned first so audio is ready before the UI loop starts.
    let i2s  = peripherals.i2s0;
    let bck  = peripherals.pins.gpio27;
    let ws   = peripherals.pins.gpio26;
    let dout = peripherals.pins.gpio25;

    thread::Builder::new()
        .stack_size(8 * 1024)
        .spawn(move || {
            if let Err(e) = audio::audio_task(i2s, bck, ws, dout) {
                log::warn!("Audio task unavailable: {:?}", e);
            }
        })?;

    // ── Display ───────────────────────────────────────────────────────────────
    let display_result = display::init_display(
        peripherals.i2c0,
        peripherals.pins.gpio21,
        peripherals.pins.gpio22,
    );
    let mut display = match display_result {
        Ok(d) => Some(d),
        Err(e) => { log::warn!("Display unavailable: {:?}", e); None }
    };

    // ── Buttons ───────────────────────────────────────────────────────────────
    let mut buttons = buttons::Buttons::new(
        peripherals.pins.gpio32,
        peripherals.pins.gpio33,
        peripherals.pins.gpio18,
        peripherals.pins.gpio19,
        peripherals.pins.gpio23,
    )?;

    log::info!("UI loop starting");

    // ── UI loop (20 Hz) ───────────────────────────────────────────────────────
    let mut tick_ms: u64 = 0;
    loop {
        buttons.poll(tick_ms);
        if let Some(ref mut d) = display { display::update(d); }
        thread::sleep(Duration::from_millis(50));
        tick_ms = tick_ms.wrapping_add(50);
    }
}
