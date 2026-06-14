/// Lock-free device state shared between the UI task (main) and audio task.
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use fp_core::{Filter, Waveform, PRESETS};

pub struct DeviceState {
    preset_idx: AtomicU32,
    playing:    AtomicBool,
    waveform:   AtomicU32,
    filter:     AtomicU32,
}

impl DeviceState {
    pub const fn new() -> Self {
        Self {
            preset_idx: AtomicU32::new(0),
            playing:    AtomicBool::new(false),
            waveform:   AtomicU32::new(0), // Waveform::Sine
            filter:     AtomicU32::new(0), // Filter::None
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
        let idx = self.preset_idx();
        self.set_preset((idx + 1) % PRESETS.len());
    }

    pub fn prev_preset(&self) {
        let idx = self.preset_idx();
        self.set_preset(if idx == 0 { PRESETS.len() - 1 } else { idx - 1 });
    }

    /// Jump to the first preset in the next category.
    pub fn next_category(&self) {
        let idx   = self.preset_idx();
        let cat   = PRESETS[idx].category;
        for i in 1..PRESETS.len() {
            let candidate = (idx + i) % PRESETS.len();
            if PRESETS[candidate].category != cat {
                self.set_preset(candidate);
                return;
            }
        }
    }

    /// Jump to the first preset in the previous category.
    pub fn prev_category(&self) {
        let idx = self.preset_idx();
        let cat = PRESETS[idx].category;
        // Walk backwards to find a different category, then to its start.
        for i in 1..PRESETS.len() {
            let back = (PRESETS.len() + idx - i) % PRESETS.len();
            if PRESETS[back].category != cat {
                let target_cat = PRESETS[back].category;
                // Rewind to the first preset of that category.
                let start = (0..=back)
                    .rev()
                    .find(|&j| PRESETS[j].category != target_cat)
                    .map(|j| j + 1)
                    .unwrap_or(0);
                self.set_preset(start);
                return;
            }
        }
    }

    pub fn current_freq(&self) -> f64 {
        PRESETS[self.preset_idx()].freq
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
}

pub static STATE: DeviceState = DeviceState::new();
