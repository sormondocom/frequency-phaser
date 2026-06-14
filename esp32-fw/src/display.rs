/// SSD1306 128×64 OLED renderer.
///
/// Hardware: Hosyond 0.96" SSD1306, I2C address 0x3C
///   SDA → GPIO 21
///   SCL → GPIO 22
///
/// Physical display layout (yellow top strip / blue main area):
///   Row 0 (y= 0..15, yellow): frequency + play indicator
///   Row 1 (y=16..27, blue):   category name
///   Row 2 (y=28..39, blue):   preset name (truncated to 21 chars)
///   Row 3 (y=40..51, blue):   waveform + filter + preset index
///   Row 4 (y=52..63, blue):   button hint
use anyhow::Result;
use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{Line, PrimitiveStyle},
    text::Text,
};
use esp_idf_hal::{
    gpio::{InputPin, OutputPin},
    i2c::{I2c, I2cConfig, I2cDriver},
    units::Hertz,
};
use ssd1306::{mode::BufferedGraphicsMode, prelude::*, I2CDisplayInterface, Ssd1306};

use crate::state::STATE;

type Display<'d> = Ssd1306<
    I2CInterface<I2cDriver<'d>>,
    DisplaySize128x64,
    BufferedGraphicsMode<DisplaySize128x64>,
>;

fn format_freq(hz: f64) -> String {
    if hz < 1.0 {
        format!("{:.3} Hz", hz)
    } else if hz < 1_000.0 {
        format!("{:.2} Hz", hz)
    } else {
        format!("{:.1} kHz", hz / 1_000.0)
    }
}

fn truncate(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        None    => s,
        Some((i, _)) => &s[..i],
    }
}

fn render(display: &mut Display<'_>) {
    let style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);

    display.clear(BinaryColor::Off).ok();

    let preset  = STATE.current_preset();
    let playing = STATE.is_playing();
    let wave    = STATE.waveform();
    let filter  = STATE.filter();
    let idx     = STATE.preset_idx();
    let total   = fp_core::PRESETS.len();

    // ── Yellow zone: frequency + status ──────────────────────────────────────
    let freq_str = format_freq(preset.freq);
    Text::new(freq_str.as_str(), Point::new(0, 10), style).draw(display).ok();

    let status = if playing { "PLAY" } else { "STOP" };
    Text::new(status, Point::new(98, 10), style).draw(display).ok();

    // Divider line at y=15 (bottom of yellow zone)
    Line::new(Point::new(0, 15), Point::new(127, 15))
        .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
        .draw(display)
        .ok();

    // ── Blue zone ─────────────────────────────────────────────────────────────
    Text::new(preset.category, Point::new(0, 26), style).draw(display).ok();
    Text::new(truncate(preset.name, 21), Point::new(0, 37), style).draw(display).ok();

    // Waveform + filter + index counter — e.g. "SINE RAW [14/67]"
    let line3 = format!("{} {} [{}/{}]", wave.label(), filter.label(), idx + 1, total);
    Text::new(&line3, Point::new(0, 48), style).draw(display).ok();

    // Button hint
    Text::new("U/D:prst  L/R:cat", Point::new(0, 59), style).draw(display).ok();

    display.flush().ok();
}

pub fn init_display<'d>(
    i2c: impl I2c + 'd,
    sda: impl InputPin + OutputPin + 'd,
    scl: impl InputPin + OutputPin + 'd,
) -> Result<Display<'d>> {
    let config = I2cConfig::new().baudrate(Hertz(400_000));
    let i2c_driver = I2cDriver::new(i2c, sda, scl, &config)?;
    let interface = I2CDisplayInterface::new(i2c_driver);
    let mut display = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
        .into_buffered_graphics_mode();
    display.init().map_err(|_| anyhow::anyhow!("SSD1306 init failed"))?;

    Ok(display)
}

pub fn update(display: &mut Display<'_>) {
    render(display);
}
