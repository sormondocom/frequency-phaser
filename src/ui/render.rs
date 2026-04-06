use std::f64::consts::PI;

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, Clear, Paragraph, Wrap,
        canvas::{Canvas, Line as CLine, Points},
    },
    Frame,
};

use crate::audio::generator::Oscillator;
use crate::presets::PRESETS;
use crate::state::{Channel, Waveform, MAX_FREQ, MIN_FREQ};
use crate::ui::app::{App, InputMode, fmt_freq, fmt_digit_zones, DIGIT_PLACE_VALUES};

// ── Colour palette ────────────────────────────────────────────────────────────

const C_ACTIVE:   Color = Color::Cyan;
const C_INACTIVE: Color = Color::DarkGray;
const C_ON:       Color = Color::Green;
const C_OFF:      Color = Color::Red;
const C_ACCENT:   Color = Color::Yellow;
const C_PRESET:   Color = Color::Yellow;
const C_BIAURAL:  Color = Color::Magenta;
const C_SCOPE:    Color = Color::Green;

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // Outer split: [main | sidebar]
    let outer = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(50), Constraint::Length(24)])
        .split(area);

    render_main(frame, app, outer[0]);
    render_sidebar(frame, app, outer[1]);

    // Modal overlays — drawn on top of everything
    match &app.mode {
        InputMode::DirectFreq { buffer } =>
            render_freq_overlay(frame, buffer, area),
        InputMode::SavePreset { freq, name_buf, .. } =>
            render_save_overlay(frame, *freq, name_buf, area),
        InputMode::DigitTune { cursor } =>
            render_digit_tune_overlay(frame, app, *cursor, area),
        InputMode::FilePathEntry { buffer, error } =>
            render_file_path_overlay(frame, buffer, error.as_deref(), area),
        _ => {}
    }
}

// ── Main panel (oscillators + scope + status) ─────────────────────────────────

fn render_main(frame: &mut Frame, app: &App, area: Rect) {
    let osc_count     = app.state.get_osc_count();
    let osc_h: u16    = 9;
    let scope_h: u16  = 7;
    let status_h: u16 = 2;

    let mut constraints: Vec<Constraint> = (0..osc_count)
        .map(|_| Constraint::Length(osc_h))
        .collect();
    constraints.push(Constraint::Min(scope_h));
    constraints.push(Constraint::Length(status_h));

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    for i in 0..osc_count {
        render_oscillator(frame, app, i, chunks[i]);
    }
    render_scope(frame, app, chunks[osc_count]);
    render_status(frame, app, chunks[osc_count + 1]);
}

// ── Oscillator panel ──────────────────────────────────────────────────────────

fn render_oscillator(frame: &mut Frame, app: &App, idx: usize, area: Rect) {
    let osc       = &app.state.oscillators[idx];
    let is_active = idx == app.active_osc;
    let enabled   = osc.is_enabled();
    let freq      = osc.get_freq();
    let waveform  = osc.get_waveform();

    let border_style = if is_active {
        Style::default().fg(C_ACTIVE).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(C_INACTIVE)
    };

    let led_color = if enabled { C_ON } else { C_OFF };
    let led       = if enabled { "●" } else { "○" };

    let filter     = osc.get_filter();
    let filt_style = if filter == crate::state::Filter::None {
        Style::default().fg(C_INACTIVE)
    } else {
        Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)
    };

    let title = Line::from(vec![
        Span::styled(format!(" OSC {} ", idx + 1), Style::default().add_modifier(Modifier::BOLD)),
        Span::styled(led, Style::default().fg(led_color)),
        Span::raw(format!("  {}  {}  {} ", fmt_freq(freq), waveform.symbol(), waveform.label())),
        Span::styled(format!("[{}]", filter.label()), filt_style),
        Span::raw(" "),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(border_style);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Split inner: [dial (20 cols) | controls]
    let inner_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(20), Constraint::Min(1)])
        .split(inner);

    render_dial(frame, freq_normalized(freq), freq, inner_chunks[0]);
    render_osc_controls(frame, app, idx, inner_chunks[1]);
}

// ── Frequency dial (canvas-based circular knob) ───────────────────────────────

fn freq_normalized(freq: f64) -> f64 {
    let log_min = MIN_FREQ.log10();
    let log_max = MAX_FREQ.log10();
    (freq.clamp(MIN_FREQ, MAX_FREQ).log10() - log_min) / (log_max - log_min)
}

fn render_dial(frame: &mut Frame, normalized: f64, freq: f64, area: Rect) {
    if area.height < 3 || area.width < 5 {
        return;
    }

    // Reserve bottom row for the frequency label
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    // Canvas: coordinate space is logical; y increases upward (standard math).
    // Terminal chars are ~2× taller than wide, so we widen x_bounds to compensate.
    let canvas = Canvas::default()
        .x_bounds([-1.6, 1.6])
        .y_bounds([-1.1, 1.1])
        .paint(move |ctx| {
            // ── Arc track (270° sweep, from 225° to −45° clockwise) ──────────
            let track_steps = 120usize;
            let mut track: Vec<(f64, f64)> = Vec::with_capacity(track_steps + 1);
            for i in 0..=track_steps {
                let t     = i as f64 / track_steps as f64;
                let angle = (225.0 - t * 270.0) * PI / 180.0;
                track.push((angle.cos(), angle.sin()));
            }
            ctx.draw(&Points { coords: &track, color: C_INACTIVE });

            // ── Active arc (from min to needle) ──────────────────────────────
            let active_steps = ((normalized * track_steps as f64) as usize).min(track_steps);
            if active_steps > 0 {
                let mut active: Vec<(f64, f64)> = Vec::with_capacity(active_steps + 1);
                for i in 0..=active_steps {
                    let t     = i as f64 / track_steps as f64;
                    let angle = (225.0 - t * 270.0) * PI / 180.0;
                    active.push((angle.cos(), angle.sin()));
                }
                ctx.draw(&Points { coords: &active, color: C_ACTIVE });
            }

            // ── Decade tick marks ─────────────────────────────────────────────
            let log_min = MIN_FREQ.log10();
            let log_max = MAX_FREQ.log10();
            let decades = [0.01f64, 0.1, 1.0, 10.0, 100.0, 1_000.0, 10_000.0, 96_000.0];
            for &f in &decades {
                let n     = (f.log10() - log_min) / (log_max - log_min);
                let angle = (225.0 - n * 270.0) * PI / 180.0;
                let (cx, cy) = (angle.cos(), angle.sin());
                ctx.draw(&Points {
                    coords: &[(cx * 1.18, cy * 1.18)],
                    color:  C_ACCENT,
                });
            }

            // ── Needle ────────────────────────────────────────────────────────
            let needle_angle = (225.0 - normalized * 270.0) * PI / 180.0;
            let nx = needle_angle.cos() * 0.80;
            let ny = needle_angle.sin() * 0.80;
            ctx.draw(&CLine { x1: 0.0, y1: 0.0, x2: nx, y2: ny, color: Color::White });

            // ── Centre dot ────────────────────────────────────────────────────
            ctx.draw(&Points { coords: &[(0.0, 0.0)], color: Color::White });
        });

    frame.render_widget(canvas, chunks[0]);

    let label = Paragraph::new(fmt_freq(freq))
        .alignment(Alignment::Center)
        .style(Style::default().fg(C_ACTIVE).add_modifier(Modifier::BOLD));
    frame.render_widget(label, chunks[1]);
}

// ── Oscillator controls (right of dial) ──────────────────────────────────────

fn render_osc_controls(frame: &mut Frame, app: &App, idx: usize, area: Rect) {
    let osc       = &app.state.oscillators[idx];
    let is_active = idx == app.active_osc;
    let amp       = osc.get_amp();
    let waveform  = osc.get_waveform();
    let channel   = osc.get_channel();
    let filter    = osc.get_filter();

    // Frequency line – show typing buffer when in direct-entry mode
    let freq_line = if is_active {
        if let InputMode::DirectFreq { buffer } = &app.mode {
            Line::from(vec![
                Span::raw("Freq: "),
                Span::styled(
                    format!("{}▌", buffer),
                    Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD),
                ),
            ])
        } else {
            Line::from(format!("Freq: {}", fmt_freq(osc.get_freq())))
        }
    } else {
        Line::from(format!("Freq: {}", fmt_freq(osc.get_freq())))
    };

    // Volume bar (10 blocks)
    let filled   = ((amp * 10.0) as usize).min(10);
    let vol_bar  = format!("{}{}", "█".repeat(filled), "░".repeat(10 - filled));
    let vol_line = Line::from(format!("Vol:  [{}] {:3.0}%", vol_bar, amp * 100.0));

    // Waveform selector
    let wave_line = waveform_line(waveform);

    // Channel selector
    let chan_line = channel_line(channel);

    // Filter selector — show loaded filename when Custom is active
    let file_name = app.state.file_name.try_lock()
        .map(|g| g.clone())
        .unwrap_or_default();
    let filt_line = filter_line(filter, &file_name);

    // Step mode (active oscillator only)
    let step_line = if is_active {
        Line::from(vec![
            Span::raw("Step: "),
            Span::styled(
                format!("[{}]", app.step_mode.label()),
                Style::default().fg(C_ACCENT),
            ),
            Span::raw("  "),
            Span::styled(
                format!("Master: {:3.0}%", app.state.get_master_vol() * 100.0),
                Style::default().fg(C_INACTIVE),
            ),
        ])
    } else {
        Line::raw("")
    };

    let style = if is_active {
        Style::default().fg(Color::White)
    } else {
        Style::default().fg(C_INACTIVE)
    };

    let text = Text::from(vec![freq_line, vol_line, wave_line, chan_line, filt_line, step_line])
        .style(style);

    frame.render_widget(Paragraph::new(text).wrap(Wrap { trim: false }), area);
}

fn waveform_line(current: Waveform) -> Line<'static> {
    // Use compact symbols so the line fits without wrapping (~26 chars vs 41 with labels)
    let mut spans = vec![Span::raw("Wave: ")];
    for &w in Waveform::all() {
        let selected = w == current;
        let style = if selected {
            Style::default()
                .fg(Color::Black)
                .bg(C_ACTIVE)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(C_INACTIVE)
        };
        spans.push(Span::styled(format!("[{}]", w.symbol()), style));
        spans.push(Span::raw(" "));
    }
    Line::from(spans)
}

fn channel_line(current: Channel) -> Line<'static> {
    let mut spans = vec![Span::raw("Chan: ")];
    for &c in Channel::all() {
        let selected = c == current;
        let style = if selected {
            Style::default()
                .fg(Color::Black)
                .bg(C_ON)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(C_INACTIVE)
        };
        spans.push(Span::styled(format!("[{}]", c.label()), style));
        spans.push(Span::raw(" "));
    }
    Line::from(spans)
}

fn filter_line(current: crate::state::Filter, file_name: &str) -> Line<'static> {
    let mut spans = vec![Span::raw("Filt: ")];
    for &f in crate::state::Filter::all() {
        let selected = f == current;
        let label = if selected && f == crate::state::Filter::Custom && !file_name.is_empty() {
            // Truncate filename to keep the line short
            let trunc = if file_name.len() > 10 { &file_name[..10] } else { file_name };
            format!("[{}]", trunc)
        } else {
            format!("[{}]", f.label())
        };
        let style = if selected {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Magenta)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(C_INACTIVE)
        };
        spans.push(Span::styled(label, style));
        spans.push(Span::raw(" "));
    }
    Line::from(spans)
}

// ── Oscilloscope ──────────────────────────────────────────────────────────────

fn render_scope(frame: &mut Frame, app: &App, area: Rect) {
    let osc_count = app.state.get_osc_count();

    // Find the lowest enabled frequency to size the time window
    let lowest = (0..osc_count)
        .filter(|&i| app.state.oscillators[i].is_enabled())
        .map(|i| app.state.oscillators[i].get_freq())
        .fold(f64::INFINITY, f64::min);

    // Show 3 cycles of the lowest frequency, clamped to a sane range
    let window_secs = if lowest.is_infinite() {
        0.02
    } else {
        (3.0 / lowest).clamp(0.001, 5.0)
    };

    let sr         = 48_000.0f64;
    let num_pts    = 512usize;
    let step_samp  = window_secs * sr / num_pts as f64;

    // Compute display waveform with temporary phase accumulators
    let mut phases: Vec<f64> = vec![0.0; osc_count];
    let mut pts: Vec<(f64, f64)> = Vec::with_capacity(num_pts);

    for i in 0..num_pts {
        let x = i as f64 / (num_pts - 1) as f64 * 100.0;
        let mut sample = 0.0f64;
        let mut active = 0usize;

        for oi in 0..osc_count {
            let os = &app.state.oscillators[oi];
            if !os.is_enabled() {
                continue;
            }
            let s = Oscillator::sample_at(phases[oi], os.get_waveform()) * os.get_amp();
            sample += s;
            active += 1;

            phases[oi] += 2.0 * PI * os.get_freq() * step_samp / sr;
            if phases[oi] >= 2.0 * PI {
                phases[oi] -= 2.0 * PI;
            }
        }

        let norm = if active > 1 { 1.0 / (active as f64).sqrt() } else { 1.0 };
        pts.push((x, (sample * norm).clamp(-1.0, 1.0) * 48.0));
    }

    let canvas = Canvas::default()
        .block(Block::default().borders(Borders::ALL).title(" Scope "))
        .x_bounds([0.0, 100.0])
        .y_bounds([-50.0, 50.0])
        .paint(move |ctx| {
            // Zero line
            ctx.draw(&CLine { x1: 0.0, y1: 0.0, x2: 100.0, y2: 0.0, color: C_INACTIVE });
            ctx.draw(&Points { coords: &pts, color: C_SCOPE });
        });

    frame.render_widget(canvas, area);
}

// ── Status bar ────────────────────────────────────────────────────────────────

fn render_status(frame: &mut Frame, app: &App, area: Rect) {
    let playing    = app.state.is_playing();
    let play_str   = if playing { "▶ PLAYING" } else { "■ STOPPED" };
    let play_color = if playing { C_ON } else { C_OFF };

    let msg = app.status_msg.as_deref().unwrap_or("");

    // Show acceleration tier when a key is held
    let accel_str = match app.arrow_repeat {
        0        => "",
        1..=19   => " ›",
        20..=39  => " ››",
        40..=79  => " »",
        80..=139 => " »»",
        _        => " »»»",
    };

    let help = "[ENTER]Play  [←→]Tune(hold=accel)  [⇧←→]Coarse  [S]Step  [PgUp/Dn]×10  \
                [W]Wave  [F]Filter  [L]Load file filter  [C]Chan  [↑↓]Vol  \
                [N]Save freq  [/]Digit tuner  [Y]Poly  [P]Presets  [F1]Add  [F2]Del  [Q]Quit";

    let line = Line::from(vec![
        Span::styled(play_str, Style::default().fg(play_color).add_modifier(Modifier::BOLD)),
        Span::styled(accel_str, Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD)),
        Span::raw("  "),
        Span::styled(msg, Style::default().fg(C_ACCENT)),
    ]);

    let help_line = Line::from(Span::styled(help, Style::default().fg(C_INACTIVE)));

    let text = Text::from(vec![line, help_line]);
    frame.render_widget(Paragraph::new(text), area);
}

// ── Sidebar (poly panel + preset list) ───────────────────────────────────────

fn render_sidebar(frame: &mut Frame, app: &App, area: Rect) {
    let poly_active = app.poly.enabled || matches!(app.mode, InputMode::PolyPanel);
    let poly_h      = if poly_active { 10u16 } else { 0 };

    if poly_active && area.height > poly_h + 3 {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(poly_h), Constraint::Min(3)])
            .split(area);
        render_poly_panel(frame, app, chunks[0]);
        render_preset_list(frame, app, chunks[1]);
    } else {
        render_preset_list(frame, app, area);
    }
}

// ── Poly panel ────────────────────────────────────────────────────────────────

fn render_poly_panel(frame: &mut Frame, app: &App, area: Rect) {
    let is_active  = matches!(app.mode, InputMode::PolyPanel);
    let poly       = &app.poly;
    let note_names = poly.note_names();
    let freqs      = poly.frequencies();

    let border_style = if is_active {
        Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(C_BIAURAL)
    };

    let title = Line::from(vec![
        Span::raw(" POLY "),
        Span::styled("[Y]", Style::default().fg(Color::Magenta)),
        Span::raw(" "),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(border_style);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Root note row
    let root_line = Line::from(vec![
        Span::raw("Root:  "),
        Span::styled(
            poly.root_name(),
            Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "  [←→] semitone  [⇧←→] octave",
            Style::default().fg(C_INACTIVE),
        ),
    ]);

    // Mode row (Chord / Scale + type)
    let kind_label = if poly.mode.is_chord() { "Chord" } else { "Scale" };
    let type_label = poly.mode.short();
    let mode_line = Line::from(vec![
        Span::raw(format!("{}: ", kind_label)),
        Span::styled(
            type_label,
            Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
        ),
        Span::styled("  [↑↓]type  [Tab]chord/scale", Style::default().fg(C_INACTIVE)),
    ]);

    // Voicing row
    let voicing_line = Line::from(vec![
        Span::raw("Voice: "),
        Span::styled(
            poly.voicing.label(),
            Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
        ),
        Span::styled("  [V]cycle", Style::default().fg(C_INACTIVE)),
    ]);

    // Separator
    let sep = Line::from(Span::styled("─────────────────────", Style::default().fg(C_INACTIVE)));

    // Notes rows — one per voice
    let mut note_lines: Vec<Line> = note_names
        .iter()
        .zip(freqs.iter())
        .enumerate()
        .map(|(i, (name, freq))| {
            let is_active_osc = i == app.active_osc;
            let marker = if is_active_osc { "►" } else { " " };
            Line::from(vec![
                Span::styled(
                    format!("{} OSC{} ", marker, i + 1),
                    Style::default().fg(if is_active_osc { C_ACTIVE } else { C_INACTIVE }),
                ),
                Span::styled(
                    format!("{:<4}", name),
                    Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" {}", fmt_freq(*freq)),
                    Style::default().fg(Color::White),
                ),
            ])
        })
        .collect();

    // Esc hint when in panel mode
    if is_active {
        note_lines.push(Line::from(Span::styled(
            "─────────────────────",
            Style::default().fg(C_INACTIVE),
        )));
        note_lines.push(Line::from(Span::styled(
            "[Esc]keep on  [Y]turn off",
            Style::default().fg(C_INACTIVE),
        )));
    }

    let mut all_lines = vec![root_line, mode_line, voicing_line, sep];
    all_lines.extend(note_lines);

    frame.render_widget(Paragraph::new(Text::from(all_lines)), inner);
}

// ── Preset list ───────────────────────────────────────────────────────────────

fn render_preset_list(frame: &mut Frame, app: &App, area: Rect) {
    let is_browsing   = matches!(&app.mode, InputMode::PresetBrowse { .. });
    let active_preset = app.current_preset;
    let n_custom      = app.custom_presets.len();

    let (browse_sel, browse_scroll) = if let InputMode::PresetBrowse { selected, scroll } = &app.mode {
        (*selected, *scroll)
    } else {
        (active_preset.unwrap_or(0), active_preset.unwrap_or(0).saturating_sub(4))
    };

    let border_style = if is_browsing {
        Style::default().fg(C_PRESET).add_modifier(Modifier::BOLD)
    } else if active_preset.is_some() {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(C_INACTIVE)
    };

    let title = if is_browsing && browse_sel < n_custom {
        " Presets [P]  [D]elete custom "
    } else {
        " Presets [P] "
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(border_style);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let visible_h = inner.height as usize;
    let scroll    = browse_scroll;
    let mut lines: Vec<Line> = Vec::new();
    let mut row   = 0usize;

    // ── Custom presets ─────────────────────────────────────────────────────
    if n_custom > 0 {
        if row >= scroll && lines.len() < visible_h {
            lines.push(Line::from(Span::styled(
                "★ Custom",
                Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
            )));
        }
        row += 1;

        for (ci, cp) in app.custom_presets.iter().enumerate() {
            if row >= scroll && lines.len() < visible_h {
                let global_idx = ci;
                let is_cursor  = is_browsing && global_idx == browse_sel;
                let is_active  = active_preset == Some(global_idx);

                let (prefix, style) = if is_cursor {
                    ("▶", Style::default().fg(Color::Black).bg(Color::Magenta))
                } else if is_active {
                    ("★", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD))
                } else {
                    ("★", Style::default().fg(Color::Yellow))
                };

                let name_trunc = if cp.name.len() > 16 { &cp.name[..16] } else { &cp.name };
                lines.push(Line::from(Span::styled(
                    format!("{} {:<16}", prefix, name_trunc),
                    style,
                )));
            }
            row += 1;
        }

        // Divider between custom and built-in
        if row >= scroll && lines.len() < visible_h {
            lines.push(Line::from(Span::styled(
                "─────────────────────",
                Style::default().fg(C_INACTIVE),
            )));
        }
        row += 1;
    }

    // ── Built-in presets ───────────────────────────────────────────────────
    let mut last_cat = "";
    for (i, preset) in PRESETS.iter().enumerate() {
        let global_idx = n_custom + i;

        if preset.category != last_cat {
            if row >= scroll && lines.len() < visible_h {
                lines.push(Line::from(Span::styled(
                    preset.category,
                    Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD),
                )));
            }
            last_cat = preset.category;
            row += 1;
        }

        if row >= scroll && lines.len() < visible_h {
            let is_cursor = is_browsing && global_idx == browse_sel;
            let is_active = active_preset == Some(global_idx);

            let (prefix, style) = if is_cursor {
                ("▶", Style::default().fg(Color::Black).bg(C_ACTIVE))
            } else if is_active {
                ("●", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD))
            } else {
                (" ", Style::default().fg(Color::White))
            };

            let name_trunc = if preset.name.len() > 18 { &preset.name[..18] } else { preset.name };
            lines.push(Line::from(Span::styled(
                format!("{} {:<18}", prefix, name_trunc),
                style,
            )));
        }
        row += 1;
    }

    // Binaural beat info
    if !app.poly.enabled {
        let osc_count = app.state.get_osc_count();
        if osc_count >= 2 {
            lines.push(Line::from(Span::raw("─────────────────────")));
            let f1   = app.state.oscillators[0].get_freq();
            let f2   = app.state.oscillators[1].get_freq();
            let diff = (f1 - f2).abs();
            lines.push(Line::from(Span::styled(
                format!("Binaural Δ {:.3} Hz", diff),
                Style::default().fg(C_BIAURAL).add_modifier(Modifier::BOLD),
            )));
        }
    }

    frame.render_widget(Paragraph::new(Text::from(lines)), inner);
}

// ── Modal overlays ────────────────────────────────────────────────────────────

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect::new(x, y, width.min(area.width), height.min(area.height))
}

/// Overlay shown during direct frequency entry.
fn render_freq_overlay(frame: &mut Frame, buffer: &str, area: Rect) {
    let popup = centered_rect(44, 7, area);
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Enter Frequency ")
        .border_style(Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD));

    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let range_line = Line::from(Span::styled(
        format!("Range: {} – {}", fmt_freq(MIN_FREQ), fmt_freq(MAX_FREQ)),
        Style::default().fg(C_INACTIVE),
    ));

    let input_line = Line::from(vec![
        Span::raw("  "),
        Span::styled(
            format!("{}_", buffer),
            Style::default()
                .fg(Color::Black)
                .bg(C_ACCENT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" Hz", Style::default().fg(C_ACCENT)),
    ]);

    let hint_line = Line::from(Span::styled(
        "  [Enter] apply   [Esc] cancel   then [N] to save",
        Style::default().fg(C_INACTIVE),
    ));

    let text = Text::from(vec![
        Line::raw(""),
        input_line,
        Line::raw(""),
        range_line,
        hint_line,
    ]);
    frame.render_widget(Paragraph::new(text), inner);
}

/// Digit-zone frequency scrubber overlay.
/// Shows `DDDDD.DDD Hz` with the active column highlighted.
/// [←→] moves cursor, [↑↓] spins the digit, [Esc//] exits.
fn render_digit_tune_overlay(frame: &mut Frame, app: &App, cursor: u8, area: Rect) {
    let popup = centered_rect(52, 10, area);
    frame.render_widget(Clear, popup);

    let freq  = app.state.oscillators[app.active_osc].get_freq();
    let zones = fmt_digit_zones(freq); // "DDDDD.DDD"

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" ◈ Digit Zone Tuner ")
        .border_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));

    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    // Build the large digit display with the cursor column highlighted.
    // zones = "DDDDD.DDD"  (9 chars incl. '.')
    // display columns:  0  1  2  3  4  .  5  6  7
    // DIGIT_PLACE_VALUES index maps to col offset skipping '.'
    let mut digit_spans: Vec<Span> = Vec::new();
    digit_spans.push(Span::raw("  "));

    let chars: Vec<char> = zones.chars().collect();
    let mut col: u8 = 0; // digit column index (skips '.')
    for &ch in &chars {
        if ch == '.' {
            digit_spans.push(Span::styled(
                "·",
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD),
            ));
            continue;
        }
        let is_cursor = col == cursor;
        let style = if is_cursor {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else if col < 5 {
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        };
        digit_spans.push(Span::styled(format!("{} ", ch), style));
        col += 1;
    }
    digit_spans.push(Span::styled(" Hz", Style::default().fg(C_ACTIVE)));

    // Place-value label for the active column
    let place = DIGIT_PLACE_VALUES[cursor as usize];
    let place_label = if place >= 1.0 {
        format!("±{} Hz per step", fmt_freq(place))
    } else {
        format!("±{} Hz per step", place)
    };

    // Zone labels above digits
    let zone_label_line = Line::from(vec![
        Span::styled(
            "  ┌ tens of kHz ──── sub-Hz ─── mHz ┐",
            Style::default().fg(C_INACTIVE),
        ),
    ]);

    let digit_line = Line::from(digit_spans);

    let active_line = Line::from(vec![
        Span::raw("  "),
        Span::styled(place_label, Style::default().fg(C_ACCENT)),
    ]);

    let hint_line = Line::from(Span::styled(
        "  [←→] zone   [↑↓] spin   [Enter] play   [N] save   [Esc] exit",
        Style::default().fg(C_INACTIVE),
    ));

    // Cursor position arrow
    let arrow_offset = 2 + (cursor as usize) * 2 + if cursor >= 5 { 1 } else { 0 };
    let arrow_line = Line::from(Span::styled(
        format!("{:width$}▲", "", width = arrow_offset),
        Style::default().fg(Color::Cyan),
    ));

    let text = Text::from(vec![
        Line::raw(""),
        zone_label_line,
        digit_line,
        arrow_line,
        active_line,
        Line::raw(""),
        hint_line,
        Line::raw(""),
    ]);
    frame.render_widget(Paragraph::new(text), inner);
}

/// Overlay for loading a WAV/MP3 file as the custom filter.
fn render_file_path_overlay(frame: &mut Frame, buffer: &str, error: Option<&str>, area: Rect) {
    let height = if error.is_some() { 13 } else { 12 };
    let popup = centered_rect(66, height, area);
    frame.render_widget(Clear, popup);

    let border_color = if error.is_some() { Color::Red } else { Color::Magenta };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Load Audio File as Filter [L] ")
        .border_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD));

    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    // Show current working directory so the user knows the base for relative paths
    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    let cwd_line = Line::from(vec![
        Span::styled("  Dir:  ", Style::default().fg(C_INACTIVE)),
        Span::styled(cwd, Style::default().fg(Color::Yellow)),
    ]);

    let input_line = Line::from(vec![
        Span::styled("  Path: ", Style::default().fg(C_INACTIVE)),
        Span::styled(
            format!("{}_", buffer),
            Style::default()
                .fg(Color::Black)
                .bg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ),
    ]);

    let mut lines = vec![
        Line::raw(""),
        cwd_line,
        Line::raw(""),
        input_line,
        Line::raw(""),
    ];

    // Error message — shown in red when the last load attempt failed
    if let Some(err) = error {
        let short = if err.len() > 58 { &err[..58] } else { err };
        lines.push(Line::from(Span::styled(
            format!("  ✗ {}", short),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::raw(""));
    }

    lines.push(Line::from(Span::styled(
        "  Supported: .wav  .mp3",
        Style::default().fg(C_INACTIVE),
    )));
    lines.push(Line::from(Span::styled(
        "  Tip: use full path (e.g. C:\\Users\\you\\Music\\song.mp3)",
        Style::default().fg(C_INACTIVE),
    )));
    lines.push(Line::from(Span::styled(
        "  [Enter] load   [Esc] cancel",
        Style::default().fg(C_INACTIVE),
    )));

    frame.render_widget(Paragraph::new(Text::from(lines)), inner);
}

/// Overlay shown when naming a custom preset to save.
fn render_save_overlay(frame: &mut Frame, freq: f64, name_buf: &str, area: Rect) {
    let popup = centered_rect(48, 8, area);
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Save Custom Preset ")
        .border_style(Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD));

    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let freq_line = Line::from(vec![
        Span::raw("  Frequency: "),
        Span::styled(
            fmt_freq(freq),
            Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
        ),
    ]);

    let display_name = if name_buf.is_empty() {
        fmt_freq(freq)
    } else {
        name_buf.to_string()
    };

    let name_line = Line::from(vec![
        Span::raw("  Name:      "),
        Span::styled(
            format!("{}_", display_name),
            Style::default()
                .fg(Color::Black)
                .bg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ),
    ]);

    let hint_line = Line::from(Span::styled(
        "  [Enter] save   [Esc] cancel",
        Style::default().fg(C_INACTIVE),
    ));
    let note_line = Line::from(Span::styled(
        "  Leave name blank to use frequency as name.",
        Style::default().fg(C_INACTIVE),
    ));

    let text = Text::from(vec![
        Line::raw(""),
        freq_line,
        name_line,
        Line::raw(""),
        hint_line,
        note_line,
    ]);
    frame.render_widget(Paragraph::new(text), inner);
}
