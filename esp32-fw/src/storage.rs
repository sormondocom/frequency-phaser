/// SPIFFS filesystem — mounted at /spiffs on internal flash.
///
/// Partition table (partitions.csv) reserves 0x2B0000 (~2.75 MB) for SPIFFS,
/// starting at offset 0x150000.  This is enough for short audio clips used as
/// playback filters while waiting for the microSD adapter.
use anyhow::Result;

const BASE: &[u8] = b"/spiffs\0";

pub fn mount() -> Result<()> {
    unsafe {
        use esp_idf_sys::{
            esp, esp_spiffs_info, esp_vfs_spiffs_conf_t, esp_vfs_spiffs_register,
        };

        let conf = esp_vfs_spiffs_conf_t {
            base_path: BASE.as_ptr() as *const core::ffi::c_char,
            partition_label: core::ptr::null(),
            max_files: 5,
            format_if_mount_failed: true,
        };
        esp!(esp_vfs_spiffs_register(&conf))?;

        let mut total: usize = 0;
        let mut used: usize = 0;
        let _ = esp_spiffs_info(core::ptr::null(), &mut total, &mut used);
        log::info!("SPIFFS mounted — {}/{} bytes used", used, total);
    }
    Ok(())
}

/// Absolute VFS path for a file stored in SPIFFS.
pub fn vfs_path(name: &str) -> String {
    format!("/spiffs/{}", name)
}

/// Cached audio file list — refreshed by `refresh_audio_files()`.
static AUDIO_FILES: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// Scan /spiffs for .mp3 and .wav files and cache the result.
/// Call this once when entering MP3 mode.
pub fn refresh_audio_files() {
    let Ok(entries) = std::fs::read_dir("/spiffs") else { return; };
    let mut files: Vec<String> = entries
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| { let lo = n.to_lowercase(); lo.ends_with(".mp3") || lo.ends_with(".wav") })
        .collect();
    files.sort();
    *AUDIO_FILES.lock().unwrap() = files;
}

pub fn audio_file_count() -> usize {
    AUDIO_FILES.lock().unwrap().len()
}

/// Returns the filename at `idx`, or an empty string if out of range.
pub fn audio_file_name(idx: usize) -> String {
    AUDIO_FILES.lock().unwrap().get(idx).cloned().unwrap_or_default()
}

/// Call `f` with a reference to the full audio file list (avoids cloning the whole Vec).
pub fn with_audio_files<F: FnOnce(&[String]) -> R, R>(f: F) -> R {
    f(&AUDIO_FILES.lock().unwrap())
}
