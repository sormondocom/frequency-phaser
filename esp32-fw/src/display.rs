/// SSD1306 128×64 OLED renderer.
///
/// Hardware: Hosyond 0.96" SSD1306, I2C address 0x3C
///   SDA → GPIO 21
///   SCL → GPIO 22
///
/// Physical display layout (yellow top strip / blue main area):
///   Row 0 (y= 0..15, yellow): frequency + play indicator
///   ── Normal mode ──────────────────────────────────────────
///   Row 1 (y=16..26, blue):   category name
///   Row 2 (y=27..37, blue):   preset name
///   Row 3 (y=38..48, blue):   waveform + filter + preset index
///   Row 4 (y=49..59, blue):   button hint
///   ── Tuning mode ──────────────────────────────────────────
///   Rows 1-3 (y=17..44, blue): waveform graphic (one cycle)
///   Row 4 (y=38..48, blue):   waveform label + filter
///   Row 5 (y=49..59, blue):   button hint
use anyhow::Result;
use embedded_graphics::{
    geometry::Size,
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{Line, PrimitiveStyle, Rectangle},
    text::Text,
};
use esp_idf_hal::{
    gpio::{InputPin, OutputPin},
    i2c::{I2c, I2cConfig, I2cDriver},
    units::Hertz,
};
use fp_core::Waveform;
use ssd1306::{mode::BufferedGraphicsMode, prelude::*, I2CDisplayInterface, Ssd1306};

use crate::state::STATE;
use crate::storage;

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
        None         => s,
        Some((i, _)) => &s[..i],
    }
}

/// Draw one cycle of `wave` as a connected line plot in the band y=17..44.
fn draw_waveform(display: &mut Display<'_>, wave: Waveform) {
    use core::f32::consts::TAU;

    const Y_TOP: i32 = 17;
    const Y_BOT: i32 = 44;
    let mid = (Y_TOP + Y_BOT) / 2;              // 30
    let amp = ((Y_BOT - Y_TOP) / 2 - 1) as f32; // 12.0

    let stroke = PrimitiveStyle::with_stroke(BinaryColor::On, 1);

    let y_of = |x: i32| -> i32 {
        let t = x as f32 / 128.0; // 0.0 → 1.0 across the display width
        let s: f32 = match wave {
            Waveform::Sine => (t * TAU).sin(),
            Waveform::Square => if t < 0.5 { 1.0 } else { -1.0 },
            Waveform::Triangle => {
                if t < 0.5 { 4.0 * t - 1.0 } else { 3.0 - 4.0 * t }
            }
            Waveform::Sawtooth => 2.0 * t - 1.0,
            Waveform::Pink => {
                // Deterministic hash: gives a fixed noisy-looking pattern
                let h = (x as u32).wrapping_mul(2_654_435_761);
                (h >> 24) as f32 / 128.0 - 1.0
            }
        };
        // s in [-1, 1]: +1 maps to y_top, -1 maps to y_bot
        (mid as f32 - s * amp) as i32
    };

    for x in 0..127i32 {
        let y1 = y_of(x).clamp(Y_TOP, Y_BOT);
        let y2 = y_of(x + 1).clamp(Y_TOP, Y_BOT);
        Line::new(Point::new(x, y1), Point::new(x + 1, y2))
            .into_styled(stroke)
            .draw(display)
            .ok();
    }
}

/// Draw the right side of the yellow zone: volume bar + PLAY/STOP + divider.
/// Called from both the oscillator and MP3 render paths so the volume
/// indicator is always visible regardless of playback mode.
fn draw_status_zone(display: &mut Display<'_>) {
    let style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);

    let status = if STATE.is_playing() { "PLAY" } else { "STOP" };
    Text::new(status, Point::new(98, 10), style).draw(display).ok();

    // Thin 2 px volume bar in the gap between text (baseline y=10) and divider (y=15).
    // Spans 0..128 px proportional to the current volume (0–100).
    let vol = STATE.volume();
    let fill_w = ((vol * 128 / 100) as u32).min(128);
    if fill_w > 0 {
        Rectangle::new(Point::new(0, 12), Size::new(fill_w, 2))
            .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
            .draw(display)
            .ok();
    }

    Line::new(Point::new(0, 15), Point::new(127, 15))
        .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
        .draw(display)
        .ok();
}

/// Draw the USB transfer screen.
fn render_transfer(display: &mut Display<'_>) {
    let style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
    let (done, total) = STATE.transfer_progress();

    display.clear(BinaryColor::Off).ok();

    Text::new("USB TRANSFER", Point::new(0, 10), style).draw(display).ok();
    Line::new(Point::new(0, 15), Point::new(127, 15))
        .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
        .draw(display).ok();

    Text::new("Receiving file...", Point::new(0, 28), style).draw(display).ok();

    // Progress bar — 120 px wide, 6 px tall at y=36
    if total > 0 {
        let filled = ((done as u64 * 120 / total as u64) as u32).min(120);
        Rectangle::new(Point::new(4, 36), Size::new(120, 6))
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
            .draw(display).ok();
        if filled > 0 {
            Rectangle::new(Point::new(4, 36), Size::new(filled, 6))
                .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
                .draw(display).ok();
        }
        let pct = done * 100 / total;
        let pct_str = format!("{:3}%  {}/{} B", pct, done, total);
        Text::new(&pct_str, Point::new(0, 52), style).draw(display).ok();
    }

    Text::new("Do not disconnect", Point::new(0, 62), style).draw(display).ok();

    display.flush().ok();
}

/// Draw the MP3 file browser screen.
///
/// Blue-zone layout (y=16..63, 48 px):
///   y=16..25  info strip  — timer (left) + selected filename (right)
///   y=27..37  file slot 1 — 11 px highlight band
///   y=38..48  file slot 2 — 11 px highlight band
///   y=50..59  hint row
fn render_mp3(display: &mut Display<'_>) {
    let style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
    let sel   = STATE.mp3_selected();

    display.clear(BinaryColor::Off).ok();

    // Yellow zone: mode label + volume bar + play/stop + divider
    Text::new("MP3 Files", Point::new(0, 10), style).draw(display).ok();
    draw_status_zone(display);

    // Info strip — elapsed timer on the left, selected filename on the right.
    // Baseline y=25 sits 10 px below the divider line (y=15).
    let elapsed = STATE.playback_elapsed_secs();
    let timer = format!("{}:{:02}", elapsed / 60, elapsed % 60);
    Text::new(&timer, Point::new(0, 25), style).draw(display).ok();

    let sel_name = storage::audio_file_name(sel);
    if !sel_name.is_empty() {
        // x=36 leaves room for up to "99:59" (5 chars × 6 px = 30 px) + 6 px gap
        Text::new(truncate(&sel_name, 15), Point::new(36, 25), style).draw(display).ok();
    }

    // File list — 2 visible slots, scrolled to keep selection in view.
    // Font 6×10 → slots at baseline y=36 and y=47 (11 px apart).
    const MAX_VISIBLE: usize = 2;
    let y_rows: [i32; 2] = [36, 47];

    storage::with_audio_files(|files| {
        if files.is_empty() {
            Text::new("No files on flash", Point::new(0, 36), style).draw(display).ok();
            Text::new("[U] to upload", Point::new(0, 47), style).draw(display).ok();
        } else {
            let scroll = if sel < 1 { 0 }
                         else if sel + 1 >= files.len() { files.len().saturating_sub(MAX_VISIBLE) }
                         else { sel - 1 };

            for (row, &y) in y_rows.iter().enumerate() {
                let file_idx = scroll + row;
                if file_idx >= files.len() { break; }
                let name = &files[file_idx];
                let is_sel = file_idx == sel;
                let display_name = truncate(name, 20);
                if is_sel {
                    Rectangle::new(Point::new(0, y - 9), Size::new(128, 11))
                        .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
                        .draw(display).ok();
                    let inv = MonoTextStyle::new(&FONT_6X10, BinaryColor::Off);
                    Text::new(display_name, Point::new(0, y), inv).draw(display).ok();
                } else {
                    Text::new(display_name, Point::new(0, y), style).draw(display).ok();
                }
            }
        }
    });

    Text::new("SEL:play  SEL+U:back", Point::new(0, 59), style).draw(display).ok();

    display.flush().ok();
}

fn render(display: &mut Display<'_>) {
    if STATE.is_transfer_active() { return render_transfer(display); }
    if STATE.is_mp3_mode()        { return render_mp3(display); }

    let style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);

    display.clear(BinaryColor::Off).ok();

    let preset = STATE.current_preset();
    let wave   = STATE.waveform();
    let filter  = STATE.filter();
    let idx     = STATE.preset_idx();
    let total   = fp_core::PRESETS.len();
    let tuning  = STATE.is_tuning();
    let freq    = STATE.current_freq();

    // ── Yellow zone: frequency + volume bar + status ─────────────────────────
    let freq_str = format_freq(freq);
    Text::new(freq_str.as_str(), Point::new(0, 10), style).draw(display).ok();
    draw_status_zone(display);

    // ── Blue zone ─────────────────────────────────────────────────────────────
    if tuning {
        // Draw the waveform shape across y=17..44
        draw_waveform(display, wave);

        // Label + hint below the graphic
        let wave_line = format!("{} / {}", wave.label(), filter.label());
        Text::new(&wave_line, Point::new(0, 48), style).draw(display).ok();
        let hint = if STATE.is_fine_step() { "L/R:1Hz  U/D:preset" }
                   else                    { "L/R:Hz   U/D:preset" };
        Text::new(hint, Point::new(0, 59), style).draw(display).ok();
    } else {
        // Normal mode: preset browser
        Text::new(preset.category, Point::new(0, 26), style).draw(display).ok();
        Text::new(truncate(preset.name, 21), Point::new(0, 37), style).draw(display).ok();
        let line3 = format!("{} {} [{}/{}]", wave.label(), filter.label(), idx + 1, total);
        Text::new(&line3, Point::new(0, 48), style).draw(display).ok();
        Text::new("U/D:prst  L/R:+/-Hz", Point::new(0, 59), style).draw(display).ok();
    }

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
