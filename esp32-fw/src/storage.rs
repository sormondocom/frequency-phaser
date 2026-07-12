/// SD card (SPI mode) mounted via ESP-IDF FAT VFS at /sdcard.
///
/// Wiring (HSPI / SPI2):
///   MOSI → GPIO 13   SCK  → GPIO 14
///   CS   → GPIO 15   MISO → GPIO 34  (input-only GPIO)
///   VCC  → 5V (module has AMS1117-3.3 onboard)   GND  → GND
///   Note: GPIO 12 is a strapping pin — never use for MISO.
use anyhow::Result;
use core::sync::atomic::{AtomicBool, Ordering};
use esp_idf_sys::EspError;

const BASE: &[u8] = b"/sdcard\0";
pub const ROOT: &str = "/sdcard";

const SD_MOSI: i32 = 13;
const SD_MISO: i32 = 34; // input-only GPIO
const SD_SCK:  i32 = 14;
const SD_CS:   i32 = 15;

// Tracks whether spi_bus_initialize has been called.
// esp_vfs_fat_sdspi_mount calls sdspi_host_deinit on failure but leaves the SPI
// bus intact, so a second mount attempt must not re-call spi_bus_initialize.
static SPI_BUS_INITIALIZED: AtomicBool = AtomicBool::new(false);

// Card handle kept alive for the process lifetime.
static mut SD_CARD: *mut esp_idf_sys::sdmmc_card_t = core::ptr::null_mut();

pub enum MountResult {
    Ok,
    NeedsFormat,
    Unavailable(anyhow::Error),
}

// ── Internal helpers ──────────────────────────────────────────────────────────

fn init_spi_bus() -> Result<(), EspError> {
    if SPI_BUS_INITIALIZED.load(Ordering::SeqCst) {
        return Ok(());
    }
    unsafe {
        use esp_idf_sys::*;
        let mut bus_cfg: spi_bus_config_t = core::mem::zeroed();
        bus_cfg.__bindgen_anon_1.mosi_io_num   = SD_MOSI;
        bus_cfg.__bindgen_anon_2.miso_io_num   = SD_MISO;
        bus_cfg.sclk_io_num                    = SD_SCK;
        bus_cfg.__bindgen_anon_3.quadwp_io_num = -1;
        bus_cfg.__bindgen_anon_4.quadhd_io_num = -1;
        bus_cfg.max_transfer_sz = 4000;
        esp!(spi_bus_initialize(
            spi_host_device_t_SPI2_HOST,
            &bus_cfg,
            spi_common_dma_t_SPI_DMA_CH_AUTO,
        ))?;
    }
    SPI_BUS_INITIALIZED.store(true, Ordering::SeqCst);
    Ok(())
}

/// Attempt to mount, optionally formatting first.
/// Returns EspError so the caller can inspect the raw code before converting.
fn try_fat_mount(format_if_needed: bool) -> Result<(), EspError> {
    use esp_idf_sys::*;
    unsafe {
        SD_CARD = core::ptr::null_mut();

        let mut host: sdmmc_host_t = core::mem::zeroed();
        host.flags        = 8; // SDMMC_HOST_FLAG_SPI = BIT(3)
        host.slot         = spi_host_device_t_SPI2_HOST as i32;
        host.max_freq_khz = 4000; // reliable on jumper wiring; raise when on PCB
        host.io_voltage   = 3.3;
        host.init              = Some(sdspi_host_init);
        host.set_card_clk      = Some(sdspi_host_set_card_clk);
        host.do_transaction    = Some(sdspi_host_do_transaction);
        // Use global deinit (not deinit_p) so sdspi_host_deinit() clears all
        // s_slots and removes all SPI bus devices — letting spi_bus_free() succeed.
        host.__bindgen_anon_1.deinit = Some(sdspi_host_deinit);
        host.io_int_enable     = Some(sdspi_host_io_int_enable);
        host.io_int_wait       = Some(sdspi_host_io_int_wait);
        host.get_real_freq     = Some(sdspi_host_get_real_freq);
        host.get_dma_info      = Some(sdspi_host_get_dma_info); // required in ESP-IDF 5.3+

        let mut slot_cfg: sdspi_device_config_t = core::mem::zeroed();
        slot_cfg.host_id  = spi_host_device_t_SPI2_HOST;
        slot_cfg.gpio_cs  = SD_CS as gpio_num_t;
        slot_cfg.gpio_cd  = -1; // GPIO_NUM_NC — no card-detect pin wired
        slot_cfg.gpio_wp  = -1; // GPIO_NUM_NC — no write-protect pin wired
        slot_cfg.gpio_int = -1; // GPIO_NUM_NC

        let mut mount_cfg: esp_vfs_fat_mount_config_t = core::mem::zeroed();
        mount_cfg.format_if_mount_failed = format_if_needed;
        mount_cfg.max_files              = 5;
        mount_cfg.allocation_unit_size   = 16 * 1024;
        // Enables CMD13 (SEND_STATUS) on every FatFS call via disk_status().
        // Makes mount_volume detect card removal in ~10 ms instead of timing out
        // on a sector read (which would block for up to 1 s).
        mount_cfg.disk_status_check_enable = true;

        esp!(esp_vfs_fat_sdspi_mount(
            BASE.as_ptr() as *const core::ffi::c_char,
            &host,
            &slot_cfg,
            &mount_cfg,
            &mut SD_CARD,
        ))?;
    }
    Ok(())
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Try to mount the SD card.  Returns `NeedsFormat` when the card is present
/// but has no recognisable FAT filesystem.
pub fn try_mount() -> MountResult {
    if let Err(e) = init_spi_bus() {
        return MountResult::Unavailable(anyhow::Error::from(e));
    }
    match try_fat_mount(false) {
        Ok(()) => {
            log::info!("SD card mounted at /sdcard");
            // Mark the card as present so poll_card_presence() sees no change on
            // the first UI tick and does not call try_mount() a second time while
            // the filesystem is already live.
            CARD_PRESENT.store(true, Ordering::Relaxed);
            MountResult::Ok
        }
        Err(e) if e.code() == -1 => MountResult::NeedsFormat,
        Err(e) => MountResult::Unavailable(anyhow::Error::from(e)),
    }
}

/// Format the SD card and mount it.  Call only after `try_mount` returns
/// `NeedsFormat`.
///
/// The failed `try_mount` already ran `sdspi_host_deinit`, which clears the
/// SDSPI device handle but leaves the SPI bus alive.  Calling `spi_bus_free`
/// here asserts because the SPI master still holds a device slot entry;
/// skip it and let `esp_vfs_fat_sdspi_mount` re-register a fresh device.
pub fn format_and_mount() -> Result<()> {
    try_fat_mount(true).map_err(anyhow::Error::from)?;
    CARD_PRESENT.store(true, Ordering::Relaxed);
    log::info!("SD card formatted and mounted at /sdcard");
    Ok(())
}

/// Absolute VFS path for a file stored on the SD card.
pub fn vfs_path(name: &str) -> String {
    format!("{}/{}", ROOT, name)
}

// ── Card presence polling ─────────────────────────────────────────────────────

static CARD_PRESENT: AtomicBool = AtomicBool::new(false);

/// Call from the UI loop (~1 Hz is plenty).  When the card is mounted, probes
/// with a real directory open so that disk_status (CMD13) fires inside
/// mount_volume — a removed card is detected in ~10 ms.  Calls
/// `unmount_sdcard()` and returns `true` when removal is detected so the
/// caller can stop playback and show the no-card screen.
///
/// When the card is not mounted (`card_present() == false`), always returns
/// `false` — the caller must probe for re-insertion by calling `try_mount()`.
pub fn poll_card_presence() -> bool {
    if !CARD_PRESENT.load(Ordering::Relaxed) {
        return false;
    }
    // Card was mounted; probe with a real VFS call so disk_status fires.
    let accessible = std::fs::read_dir(ROOT).is_ok();
    if !accessible {
        unmount_sdcard();
        CARD_PRESENT.store(false, Ordering::Relaxed);
        log::warn!("SD card removed — unmounted /sdcard");
        return true; // state changed: was present, now absent
    }
    false
}

/// Unmount the FAT filesystem and release SDSPI resources.  The SPI bus
/// itself stays initialised so the next `try_mount()` call can skip
/// `spi_bus_initialize`.  Safe to call even if the card is physically absent
/// (unmount_card_core does not communicate with the card).
pub fn unmount_sdcard() {
    unsafe {
        if !SD_CARD.is_null() {
            let _ = esp_idf_sys::esp_vfs_fat_sdcard_unmount(
                BASE.as_ptr() as *const core::ffi::c_char,
                SD_CARD,
            );
            SD_CARD = core::ptr::null_mut();
        }
    }
}

pub fn card_present() -> bool {
    CARD_PRESENT.load(Ordering::Relaxed)
}

/// Cached audio file list — refreshed by `refresh_audio_files()`.
static AUDIO_FILES: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// Scan /sdcard for .mp3 and .wav files and cache the result.
/// Call this once when entering MP3 mode.
pub fn refresh_audio_files() {
    let Ok(entries) = std::fs::read_dir(ROOT) else { return; };
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
