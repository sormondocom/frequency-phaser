use std::f64::consts::PI;

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, Paragraph, Wrap,
        canvas::{Canvas, Line as CLine, Points},
    },
    Frame,
};

use crate::audio::generator::Oscillator;
use crate::presets::PRESETS;
use crate::state::{Channel, Waveform, MAX_FREQ, MIN_FREQ};
use crate::ui::app::{App, InputMode, fmt_freq};

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

    // Filter selector
    let filt_line = filter_line(filter);

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

fn filter_line(current: crate::state::Filter) -> Line<'static> {
    let mut spans = vec![Span::raw("Filt: ")];
    for &f in crate::state::Filter::all() {
        let selected = f == current;
        let style = if selected {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Magenta)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(C_INACTIVE)
        };
        spans.push(Span::styled(format!("[{}]", f.label()), style));
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

    let help = "[ENTER]Play  [←→]Tune  [S]Step  [PgUp/Dn]×10  [W]Wave  [F]Filter  \
                [C]Chan  [↑↓]Vol  [Y]Poly  [P]Presets  [F1]Add  [F2]Del  [Q]Quit";

    let line = Line::from(vec![
        Span::styled(play_str, Style::default().fg(play_color).add_modifier(Modifier::BOLD)),
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

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Presets [P] ")
        .border_style(border_style);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let visible_h = inner.height as usize;
    let anchor    = if is_browsing { browse_sel } else { active_preset.unwrap_or(0) };
    let scroll    = {
        let raw = browse_scroll;
        if anchor >= raw + visible_h { anchor - visible_h + 1 } else { raw.min(anchor) }
    };

    let mut lines: Vec<Line> = Vec::new();
    let mut last_cat = "";
    let mut row      = 0usize;

    for (i, preset) in PRESETS.iter().enumerate() {
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
            let is_cursor = is_browsing && i == browse_sel;
            let is_active = active_preset == Some(i);

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

    // Binaural beat info — only when poly is off (poly panel already shows notes)
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
