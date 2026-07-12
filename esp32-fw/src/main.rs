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
/// MicroSD TF adapter (SPI2 / HSPI)
///   MOSI → GPIO 13    SCK  → GPIO 14
///   CS   → GPIO 15    MISO → GPIO 34   (input-only)
///   VCC  → 5V (module has AMS1117-3.3 onboard)   GND → GND
///   Note: GPIO 12 is a strapping pin — do not use for MISO
///
/// Tactile buttons (active-low, internal pull-up)
///   UP     → GPIO 32  DOWN   → GPIO 33
///   LEFT   → GPIO 18  RIGHT  → GPIO 19
///   SELECT → GPIO 23
///   VOL+   → GPIO 4   VOL-   → GPIO 5
/// ─────────────────────────────────────────────────────────────
use anyhow::Result;
use esp_idf_hal::peripherals::Peripherals;
use std::{thread, time::Duration};

mod audio;
mod buttons;
mod display;
mod state;
mod storage;
mod upload;

fn main() -> Result<()> {
    // Required esp-idf glue — must be the very first call.
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    unsafe {
        esp_idf_sys::esp_log_level_set(
            b"i2c\0".as_ptr() as *const core::ffi::c_char,
            esp_idf_sys::esp_log_level_t_ESP_LOG_ERROR,
        );
    }

    log::info!("Frequency Phaser ESP32 starting");

    // Upload listener uses raw UART — no HAL peripheral needed.
    thread::Builder::new()
        .stack_size(16 * 1024)
        .spawn(|| upload::run_listener())?;

    let peripherals = Peripherals::take()?;

    // ── Display and buttons init first ────────────────────────────────────────
    // Both are needed before the SD mount attempt so the format confirmation
    // screen can be shown if the card has no filesystem.
    let display_result = display::init_display(
        peripherals.i2c0,
        peripherals.pins.gpio21,
        peripherals.pins.gpio22,
    );
    let mut display = match display_result {
        Ok(d)  => Some(d),
        Err(e) => { log::warn!("Display unavailable: {:?}", e); None }
    };

    let mut buttons = buttons::Buttons::new(
        peripherals.pins.gpio32,
        peripherals.pins.gpio33,
        peripherals.pins.gpio18,
        peripherals.pins.gpio19,
        peripherals.pins.gpio23,
        peripherals.pins.gpio4,
        peripherals.pins.gpio5,
    )?;

    // ── SD card mount ─────────────────────────────────────────────────────────
    // No card → boot normally (oscillator works without SD).
    // Card present but unformatted → block until formatted or physically removed.
    'sd_boot: loop {
        match storage::try_mount() {
            storage::MountResult::Ok => break 'sd_boot,

            storage::MountResult::Unavailable(e) => {
                log::info!("No SD card detected ({:?}) — booting without storage", e);
                break 'sd_boot;
            }

            storage::MountResult::NeedsFormat => {
                if let Some(ref mut d) = display { display::render_format_prompt(d); }
                let confirmed = loop {
                    if buttons.select_held() { thread::sleep(Duration::from_millis(300)); break true; }
                    if buttons.left_held()   { thread::sleep(Duration::from_millis(300)); break false; }
                    thread::sleep(Duration::from_millis(50));
                };
                if confirmed {
                    // Run format on a background thread so the main thread can
                    // animate the display — format_and_mount() has no progress callback.
                    let format_done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                    let flag = format_done.clone();
                    let format_handle = thread::Builder::new()
                        .stack_size(8 * 1024)
                        .spawn(move || {
                            let r = storage::format_and_mount();
                            flag.store(true, std::sync::atomic::Ordering::Relaxed);
                            r
                        })
                        .expect("format thread");
                    let mut tick = 0u32;
                    while !format_done.load(std::sync::atomic::Ordering::Relaxed) {
                        if let Some(ref mut d) = display { display::render_formatting(d, tick); }
                        tick += 1;
                        thread::sleep(Duration::from_millis(250));
                    }
                    let result = format_handle.join().unwrap_or_else(|_| Err(anyhow::anyhow!("format thread panicked")));
                    match result {
                        Ok(()) => break 'sd_boot,
                        Err(e) => {
                            log::warn!("SD format failed: {:?}", e);
                            if let Some(ref mut d) = display { display::render_format_error(d); }
                            thread::sleep(Duration::from_secs(3));
                            // Loop back — show prompt again
                        }
                    }
                } else {
                    // User declined — wait until the card is physically removed,
                    // then boot without SD.
                    if let Some(ref mut d) = display { display::render_remove_card(d); }
                    loop {
                        thread::sleep(Duration::from_secs(2));
                        if let storage::MountResult::Unavailable(_) = storage::try_mount() {
                            break 'sd_boot;
                        }
                        // Card still present — keep showing remove-card screen
                    }
                }
            }
        }
    }

    // ── Audio thread ──────────────────────────────────────────────────────────
    let i2s  = peripherals.i2s0;
    let bck  = peripherals.pins.gpio27;
    let ws   = peripherals.pins.gpio26;
    let dout = peripherals.pins.gpio25;

    thread::Builder::new()
        .stack_size(32 * 1024)
        .spawn(move || {
            if let Err(e) = audio::audio_task(i2s, bck, ws, dout) {
                log::warn!("Audio task unavailable: {:?}", e);
            }
        })?;

    // ── UI loop (20 Hz) ───────────────────────────────────────────────────────
    // `card_absent` is set only when a mounted card is removed at runtime.
    // Booting without an SD card leaves it false (oscillator works fine without SD).
    let mut card_absent = false;
    let mut tick_ms: u64 = 0;
    let mut card_poll_tick: u64 = 0;
    let mut mount_retry_tick: u64 = 0;
    loop {
        buttons.poll(tick_ms);

        // Detect card removal at ~1 Hz.  poll_card_presence() returns true only
        // when a previously-mounted card disappears; it also calls unmount_sdcard()
        // internally so try_mount() can succeed on re-insertion.
        if !card_absent && tick_ms.wrapping_sub(card_poll_tick) >= 1000 {
            card_poll_tick = tick_ms;
            if storage::poll_card_presence() {
                // Card removed.
                card_absent = true;
                mount_retry_tick = tick_ms; // start 3 s countdown before first re-insert probe
                log::warn!("SD card removed");
                state::STATE.stop_playing();
                state::STATE.exit_mp3_mode();
                if let Some(ref mut d) = display { display::render_no_card(d); }
            }
        }

        // Probe for card re-insertion every 3 s while card is absent.
        if card_absent && tick_ms.wrapping_sub(mount_retry_tick) >= 3000 {
            mount_retry_tick = tick_ms;
            match storage::try_mount() {
                storage::MountResult::Ok => {
                    card_absent = false;
                    log::warn!("SD card re-inserted and mounted at /sdcard");
                    if let Some(ref mut d) = display { display::render_card_inserted(d); }
                    thread::sleep(Duration::from_millis(800));
                    storage::refresh_audio_files();
                }
                storage::MountResult::NeedsFormat => {
                    if let Some(ref mut d) = display { display::render_format_prompt(d); }
                    let confirmed = loop {
                        if buttons.select_held() { thread::sleep(Duration::from_millis(300)); break true; }
                        if buttons.left_held()   { thread::sleep(Duration::from_millis(300)); break false; }
                        thread::sleep(Duration::from_millis(50));
                    };
                    if confirmed {
                        if let Some(ref mut d) = display { display::render_formatting(d, 0); }
                        match storage::format_and_mount() {
                            Ok(()) => { card_absent = false; }
                            Err(e) => {
                                log::warn!("SD format failed: {:?}", e);
                                if let Some(ref mut d) = display { display::render_format_error(d); }
                                thread::sleep(Duration::from_secs(3));
                                if let Some(ref mut d) = display { display::render_no_card(d); }
                            }
                        }
                    } else {
                        // User declined format — keep showing no-card screen.
                        if let Some(ref mut d) = display { display::render_no_card(d); }
                    }
                }
                storage::MountResult::Unavailable(_) => {
                    // Card still absent — try again next interval.
                }
            }
        }

        // Skip normal display update while card is absent so the no-card screen persists.
        if !card_absent {
            if let Some(ref mut d) = display { display::update(d); }
        }
        thread::sleep(Duration::from_millis(50));
        tick_ms = tick_ms.wrapping_add(50);
    }
}
