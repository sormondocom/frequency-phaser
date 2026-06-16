/// Serial UART file upload to the ESP32 firmware over USB-serial.
///
/// Protocol:
///   Desktop → ESP32 : "FPUPLOAD:<filename>:<byte_count>\n"
///   ESP32   → Desktop: "READY\n"
///   Desktop → ESP32 : raw bytes in 256-byte chunks
///   ESP32   → Desktop: "ACK\n"  per chunk
///   ESP32   → Desktop: "OK:<n>\n" when complete, or "ERR:<msg>\n"
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct UploadProgress {
    pub sent:    Arc<AtomicUsize>,
    pub total:   Arc<AtomicUsize>,
    pub done:    Arc<AtomicBool>,
    pub result:  Arc<std::sync::Mutex<Option<Result<usize, String>>>>,
}

impl UploadProgress {
    pub fn new() -> Self {
        Self {
            sent:   Arc::new(AtomicUsize::new(0)),
            total:  Arc::new(AtomicUsize::new(0)),
            done:   Arc::new(AtomicBool::new(false)),
            result: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    pub fn fraction(&self) -> f64 {
        let t = self.total.load(Ordering::Relaxed);
        if t == 0 { return 0.0; }
        self.sent.load(Ordering::Relaxed) as f64 / t as f64
    }
}

/// List available serial ports.  Returns port names (e.g. "COM5", "/dev/ttyUSB0").
pub fn list_ports() -> Vec<String> {
    serialport::available_ports()
        .unwrap_or_default()
        .into_iter()
        .map(|p| p.port_name)
        .collect()
}

/// Spawn a background thread that transfers `file_path` to the ESP32 via `port_name`.
/// Progress and result are reported through the returned `UploadProgress`.
pub fn start_upload(port_name: String, file_path: String) -> UploadProgress {
    let progress = UploadProgress::new();

    let sent_a   = Arc::clone(&progress.sent);
    let total_a  = Arc::clone(&progress.total);
    let done_a   = Arc::clone(&progress.done);
    let result_a = Arc::clone(&progress.result);

    std::thread::spawn(move || {
        let outcome = do_upload(&port_name, &file_path, &sent_a, &total_a);
        *result_a.lock().unwrap() = Some(outcome.map_err(|e| e.to_string()));
        done_a.store(true, Ordering::Relaxed);
    });

    progress
}

fn do_upload(
    port_name: &str,
    file_path: &str,
    sent_a:  &AtomicUsize,
    total_a: &AtomicUsize,
) -> anyhow::Result<usize> {
    let data = std::fs::read(file_path)?;
    let total = data.len();
    total_a.store(total, Ordering::Relaxed);

    let filename = Path::new(file_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("upload.bin");

    let mut port = serialport::new(port_name, 115_200)
        .timeout(Duration::from_secs(15))
        .open()
        .map_err(|e| anyhow::anyhow!("Cannot open {}: {}", port_name, e))?;

    // Opening the port asserts DTR, which on most ESP32 dev boards triggers an automatic
    // reset via the EN pin.  Deassert both lines and wait for the device to finish booting
    // before sending anything — otherwise the header arrives while the firmware is still
    // coming up and the upload listener never sees it.
    port.write_data_terminal_ready(false).ok();
    port.write_request_to_send(false).ok();
    std::thread::sleep(Duration::from_millis(1500));

    // Drain any boot noise that accumulated during the wait.
    let mut trash = [0u8; 256];
    while port.bytes_to_read().unwrap_or(0) > 0 {
        let _ = port.read(&mut trash);
    }

    // Send handshake
    let header = format!("FPUPLOAD:{}:{}\n", filename, total);
    port.write_all(header.as_bytes())?;
    port.flush()?;

    // Wait for READY
    let mut reader = BufReader::new(port.try_clone()?);
    wait_for(&mut reader, "READY")?;

    // Send chunks
    const CHUNK: usize = 256;
    let mut offset = 0;
    while offset < total {
        let end = (offset + CHUNK).min(total);
        port.write_all(&data[offset..end])?;
        port.flush()?;
        wait_for(&mut reader, "ACK")?;
        offset = end;
        sent_a.store(offset, Ordering::Relaxed);
    }

    // Read final OK:<n>
    let reply = read_line(&mut reader)?;
    if let Some(n_str) = reply.trim().strip_prefix("OK:") {
        Ok(n_str.trim().parse().unwrap_or(total))
    } else {
        anyhow::bail!("Unexpected final response: {:?}", reply)
    }
}

fn wait_for<R: std::io::Read>(reader: &mut BufReader<R>, token: &str) -> anyhow::Result<()> {
    let mut line = String::new();
    loop {
        line.clear();
        reader.read_line(&mut line)?;
        if line.trim().contains(token) {
            return Ok(());
        }
        // Skip log noise — the ESP32 may emit stray log lines before silencing itself.
    }
}

fn read_line<R: std::io::Read>(reader: &mut BufReader<R>) -> anyhow::Result<String> {
    let mut line = String::new();
    reader.read_line(&mut line)?;
    Ok(line)
}
