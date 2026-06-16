/// Lock-free device state shared between the UI task (main) and audio task.
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use fp_core::{Filter, Waveform, PRESETS};

pub struct DeviceState {
    preset_idx:        AtomicU32,
    playing:           AtomicBool,
    waveform:          AtomicU32,
    filter:            AtomicU32,
    volume:            AtomicU32,  // 0–100
    tuning:            AtomicBool, // true = L/R Hz-adjust mode active
    custom_freq_bits:  AtomicU32,  // f32 bits of user-adjusted frequency
    fine_step:         AtomicBool, // true = 1 Hz step (hold-one-press-other gesture)
    mp3_mode:          AtomicBool, // true = MP3 file browser active
    mp3_selected:      AtomicU32,  // index into the SPIFFS file list
    transfer_active:   AtomicBool, // true = USB file transfer in progress
    transfer_total:    AtomicU32,  // file size being received
    transfer_done:     AtomicU32,  // bytes received so far
    playback_samples:  AtomicU32,  // I2S frames written since last play-start
    playback_total:    AtomicU32,  // estimated total duration of current MP3 (seconds)
}

impl DeviceState {
    pub const fn new() -> Self {
        Self {
            preset_idx:       AtomicU32::new(0),
            playing:          AtomicBool::new(false),
            waveform:         AtomicU32::new(0), // Waveform::Sine
            filter:           AtomicU32::new(0), // Filter::None
            volume:           AtomicU32::new(75),
            tuning:           AtomicBool::new(false),
            custom_freq_bits: AtomicU32::new(0),
            fine_step:        AtomicBool::new(false),
            mp3_mode:         AtomicBool::new(false),
            mp3_selected:     AtomicU32::new(0),
            transfer_active:  AtomicBool::new(false),
            transfer_total:   AtomicU32::new(0),
            transfer_done:    AtomicU32::new(0),
            playback_samples: AtomicU32::new(0),
            playback_total:   AtomicU32::new(0),
        }
    }

    // ── Preset navigation ─────────────────────────────────────────────────────

    pub fn preset_idx(&self) -> usize {
        self.preset_idx.load(Ordering::Relaxed) as usize
    }

    fn set_preset(&self, idx: usize) {
        self.preset_idx.store(idx.min(PRESETS.len() - 1) as u32, Ordering::Relaxed);
    }

    pub fn next_preset(&self) {
        self.exit_tuning();
        let idx = self.preset_idx();
        self.set_preset((idx + 1) % PRESETS.len());
    }

    pub fn prev_preset(&self) {
        self.exit_tuning();
        let idx = self.preset_idx();
        self.set_preset(if idx == 0 { PRESETS.len() - 1 } else { idx - 1 });
    }

    pub fn current_freq(&self) -> f64 {
        if self.tuning.load(Ordering::Relaxed) {
            f32::from_bits(self.custom_freq_bits.load(Ordering::Relaxed)) as f64
        } else {
            PRESETS[self.preset_idx()].freq
        }
    }

    pub fn current_preset(&self) -> &'static fp_core::Preset {
        &PRESETS[self.preset_idx()]
    }

    // ── Playback ──────────────────────────────────────────────────────────────

    pub fn is_playing(&self) -> bool {
        self.playing.load(Ordering::Relaxed)
    }

    pub fn toggle_playing(&self) -> bool {
        let was = self.playing.load(Ordering::Relaxed);
        self.playing.store(!was, Ordering::Relaxed);
        !was
    }

    pub fn stop_playing(&self) {
        self.playing.store(false, Ordering::Relaxed);
    }

    // ── Waveform & filter ─────────────────────────────────────────────────────

    pub fn waveform(&self) -> Waveform {
        Waveform::from_u32(self.waveform.load(Ordering::Relaxed))
    }

    pub fn next_waveform(&self) {
        let w = self.waveform().next();
        self.waveform.store(w as u32, Ordering::Relaxed);
    }

    pub fn filter(&self) -> Filter {
        Filter::from_u32(self.filter.load(Ordering::Relaxed))
    }

    pub fn next_filter(&self) {
        let f = self.filter().next();
        self.filter.store(f as u32, Ordering::Relaxed);
    }

    // ── Volume ────────────────────────────────────────────────────────────────

    pub fn volume(&self) -> u32 {
        self.volume.load(Ordering::Relaxed)
    }

    pub fn vol_up(&self) {
        let v = self.volume().saturating_add(5).min(100);
        self.volume.store(v, Ordering::Relaxed);
    }

    pub fn vol_down(&self) {
        let v = self.volume().saturating_sub(5);
        self.volume.store(v, Ordering::Relaxed);
    }

    // ── Hz tuning ─────────────────────────────────────────────────────────────

    pub fn is_tuning(&self) -> bool {
        self.tuning.load(Ordering::Relaxed)
    }

    /// Nudge frequency up or down by an adaptive step.
    /// First call initializes from the current preset frequency.
    pub fn freq_step(&self, up: bool) {
        let base = if !self.tuning.load(Ordering::Relaxed) {
            let f = PRESETS[self.preset_idx()].freq as f32;
            self.custom_freq_bits.store(f.to_bits(), Ordering::Relaxed);
            self.tuning.store(true, Ordering::Relaxed);
            f
        } else {
            f32::from_bits(self.custom_freq_bits.load(Ordering::Relaxed))
        };

        let step: f32 = if self.fine_step.load(Ordering::Relaxed) {
            1.0
        } else if base < 1.0       { 0.01 }
          else if base < 10.0      { 0.1  }
          else if base < 100.0     { 1.0  }
          else if base < 1_000.0   { 5.0  }
          else                     { 50.0 };

        let new_freq = if up {
            (base + step).min(96_000.0)
        } else {
            (base - step).max(0.01)
        };
        self.custom_freq_bits.store(new_freq.to_bits(), Ordering::Relaxed);
    }

    /// Round the current frequency to the nearest whole Hz.
    /// Enters tuning mode if not already in it (seeds from the current preset).
    pub fn round_freq(&self) {
        let freq = self.current_freq() as f32;
        let rounded = freq.round().max(1.0);
        self.custom_freq_bits.store(rounded.to_bits(), Ordering::Relaxed);
        self.tuning.store(true, Ordering::Relaxed);
    }

    pub fn is_fine_step(&self) -> bool {
        self.fine_step.load(Ordering::Relaxed)
    }

    pub fn toggle_fine_step(&self) {
        let was = self.fine_step.load(Ordering::Relaxed);
        self.fine_step.store(!was, Ordering::Relaxed);
    }

    fn exit_tuning(&self) {
        self.tuning.store(false, Ordering::Relaxed);
        self.fine_step.store(false, Ordering::Relaxed);
    }

    // ── USB transfer ──────────────────────────────────────────────────────────

    pub fn start_transfer(&self, total: usize) {
        self.transfer_total.store(total as u32, Ordering::Relaxed);
        self.transfer_done.store(0, Ordering::Relaxed);
        self.transfer_active.store(true, Ordering::Relaxed);
    }

    pub fn update_transfer(&self, done: usize) {
        self.transfer_done.store(done as u32, Ordering::Relaxed);
    }

    pub fn end_transfer(&self) {
        self.transfer_active.store(false, Ordering::Relaxed);
    }

    pub fn is_transfer_active(&self) -> bool {
        self.transfer_active.load(Ordering::Relaxed)
    }

    pub fn transfer_progress(&self) -> (u32, u32) {
        (
            self.transfer_done.load(Ordering::Relaxed),
            self.transfer_total.load(Ordering::Relaxed),
        )
    }

    // ── MP3 file browser ──────────────────────────────────────────────────────

    pub fn is_mp3_mode(&self) -> bool {
        self.mp3_mode.load(Ordering::Relaxed)
    }

    pub fn enter_mp3_mode(&self) {
        self.mp3_mode.store(true, Ordering::Relaxed);
        self.mp3_selected.store(0, Ordering::Relaxed);
    }

    pub fn exit_mp3_mode(&self) {
        self.mp3_mode.store(false, Ordering::Relaxed);
    }

    pub fn mp3_selected(&self) -> usize {
        self.mp3_selected.load(Ordering::Relaxed) as usize
    }

    // ── MP3 playback timer ────────────────────────────────────────────────────

    /// Call when a new file is opened so the timer restarts from 0.
    pub fn reset_playback_timer(&self) {
        self.playback_samples.store(0, Ordering::Relaxed);
    }

    /// Called by the audio task each time a buffer of `frames` samples is
    /// written to I2S during MP3 playback.
    pub fn advance_playback(&self, frames: u32) {
        let cur = self.playback_samples.load(Ordering::Relaxed);
        self.playback_samples.store(cur.saturating_add(frames), Ordering::Relaxed);
    }

    /// Elapsed playback time in whole seconds (44 100 samples/s).
    pub fn playback_elapsed_secs(&self) -> u32 {
        self.playback_samples.load(Ordering::Relaxed) / 44_100
    }

    /// Set estimated total duration from the first decoded MP3 frame.
    pub fn set_playback_total(&self, secs: u32) {
        self.playback_total.store(secs, Ordering::Relaxed);
    }

    /// Estimated total song duration in seconds (0 = unknown / not yet decoded).
    pub fn playback_total_secs(&self) -> u32 {
        self.playback_total.load(Ordering::Relaxed)
    }

    /// Move selection down (wraps). `count` is the current file list length.
    pub fn mp3_next(&self, count: usize) {
        if count == 0 { return; }
        let cur = self.mp3_selected();
        self.mp3_selected.store(((cur + 1) % count) as u32, Ordering::Relaxed);
    }

    /// Move selection up (wraps). `count` is the current file list length.
    pub fn mp3_prev(&self, count: usize) {
        if count == 0 { return; }
        let cur = self.mp3_selected();
        let prev = if cur == 0 { count - 1 } else { cur - 1 };
        self.mp3_selected.store(prev as u32, Ordering::Relaxed);
    }
}

pub static STATE: DeviceState = DeviceState::new();
