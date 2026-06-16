/// UART upload receiver — reads from UART0 hardware directly via uart_read_bytes,
/// bypassing the VFS stdin layer which is not reliably connected to UART0 RX.
///
/// Protocol:
///   FPUPLOAD:<name>:<bytes>\n  → READY\n → (chunks) → ACK\n×N → OK:<n>\n|ERR:<msg>\n
///   FPLIST\n                   → <name>\n×N → END\n
///   FPDELETE:<name>\n          → OK\n | ERR:<msg>\n
use std::io::Write as _;

const UART: u32 = 0;

pub fn run_listener() -> ! {
    // uart_read_bytes requires the interrupt-driven UART driver to be installed.
    // tx_buffer_size=0 keeps TX in direct/polling mode so existing log output is unaffected.
    unsafe {
        esp_idf_sys::uart_driver_install(
            UART,
            1024, // RX ring buffer
            0,    // TX: direct writes, no buffer
            0,    // no event queue
            core::ptr::null_mut(),
            0,
        );
    }

    let mut line = String::with_capacity(128);
    loop {
        line.clear();
        if read_line(&mut line) {
            let trimmed = line.trim_end_matches(|c| c == '\r' || c == '\n');
            if let Some(args) = trimmed.strip_prefix("FPUPLOAD:") {
                receive_file(args);
            } else if trimmed == "FPLIST" {
                list_files();
            } else if let Some(name) = trimmed.strip_prefix("FPDELETE:") {
                delete_file(name);
            }
        }
    }
}

/// Read UART0 bytes into `buf` until '\n'.  Returns true when a line is complete.
/// Times out (returns false) after 200 ms of silence — outer loop retries.
fn read_line(buf: &mut String) -> bool {
    loop {
        match uart_read_byte(200) {
            None        => return false,
            Some(b'\r') => {}
            Some(b'\n') => return !buf.is_empty(),
            Some(b)     => buf.push(b as char),
        }
    }
}

fn receive_file(args: &str) {
    let Some((raw_name, size_str)) = args.split_once(':') else { return; };
    let Ok(total) = size_str.trim().parse::<usize>() else { return; };

    let safe_name: String = raw_name
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, '.' | '-' | '_'))
        .take(32)
        .collect();
    if safe_name.is_empty() { return; }

    crate::state::STATE.start_transfer(total);
    log::set_max_level(log::LevelFilter::Off);

    let path = crate::storage::vfs_path(&safe_name);
    let result = write_file(&path, total);

    match result {
        Ok(n)  => uart_write(format!("OK:{}\n", n).as_bytes()),
        Err(e) => uart_write(format!("ERR:{}\n", e).as_bytes()),
    }

    log::set_max_level(log::LevelFilter::Warn);
    crate::state::STATE.end_transfer();
    log::warn!("upload: saved {} ({} bytes)", safe_name, total);
}

fn write_file(path: &str, total: usize) -> anyhow::Result<usize> {
    uart_write(b"READY\n");

    let mut file = std::fs::File::create(path)?;
    let mut buf = [0u8; 256];
    let mut received = 0usize;

    while received < total {
        let chunk = (total - received).min(256);
        if !uart_read_exact(&mut buf[..chunk]) {
            anyhow::bail!("timeout at byte {}", received);
        }
        file.write_all(&buf[..chunk])?;
        received += chunk;
        crate::state::STATE.update_transfer(received);
        uart_write(b"ACK\n");
    }

    file.flush()?;
    Ok(received)
}

// ── File management commands ──────────────────────────────────────────────────

/// FPLIST — respond with one filename per line, terminated by "END\n".
fn list_files() {
    log::set_max_level(log::LevelFilter::Off);
    match std::fs::read_dir("/spiffs") {
        Ok(entries) => {
            for entry in entries.flatten() {
                if let Ok(name) = entry.file_name().into_string() {
                    uart_write(format!("{}\n", name).as_bytes());
                }
            }
        }
        Err(_) => {}
    }
    uart_write(b"END\n");
    log::set_max_level(log::LevelFilter::Warn);
}

/// FPDELETE:<filename> — delete the named file, respond "OK\n" or "ERR:<msg>\n".
fn delete_file(raw_name: &str) {
    let safe_name: String = raw_name
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, '.' | '-' | '_'))
        .take(32)
        .collect();
    if safe_name.is_empty() {
        uart_write(b"ERR:invalid name\n");
        return;
    }
    log::set_max_level(log::LevelFilter::Off);
    let path = crate::storage::vfs_path(&safe_name);
    match std::fs::remove_file(&path) {
        Ok(())  => {
            uart_write(b"OK\n");
            // Refresh the in-memory audio file list so the MP3 browser stays consistent.
            crate::storage::refresh_audio_files();
        }
        Err(e) => uart_write(format!("ERR:{}\n", e).as_bytes()),
    }
    log::set_max_level(log::LevelFilter::Warn);
}

// ── UART0 primitives ──────────────────────────────────────────────────────────

fn uart_read_byte(timeout_ms: u32) -> Option<u8> {
    let mut b = 0u8;
    let n = unsafe {
        esp_idf_sys::uart_read_bytes(
            UART,
            &mut b as *mut u8 as *mut core::ffi::c_void,
            1,
            timeout_ms,
        )
    };
    if n == 1 { Some(b) } else { None }
}

/// Read exactly `buf.len()` bytes from UART0.  Each call to uart_read_bytes
/// has a 2-second timeout; the loop retries until the full chunk arrives.
fn uart_read_exact(buf: &mut [u8]) -> bool {
    let mut pos = 0;
    while pos < buf.len() {
        let n = unsafe {
            esp_idf_sys::uart_read_bytes(
                UART,
                buf[pos..].as_mut_ptr() as *mut core::ffi::c_void,
                (buf.len() - pos) as u32,
                2000, // 2-second timeout per partial read
            )
        };
        if n <= 0 { return false; }
        pos += n as usize;
    }
    true
}

fn uart_write(data: &[u8]) {
    unsafe {
        esp_idf_sys::uart_write_bytes(
            UART,
            data.as_ptr() as *const core::ffi::c_void,
            data.len(),
        );
    }
}
