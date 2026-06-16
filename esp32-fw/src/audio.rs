/// I2S audio output task — runs on a dedicated high-priority thread.
///
/// Hardware: PCM5102A DAC
///   BCK  → GPIO 27   (Bit Clock)
///   LRCK → GPIO 26   (Word Select / LR Clock)
///   DIN  → GPIO 25   (Data In to DAC)
///   FMT  → GND       (I2S format, not DSP)
///   XSMT → 3V3       (soft-mute disabled = audio active)
///   SCK  → GND       (no MCLK; PCM5102A derives clock from BCK)
use anyhow::Result;
use esp_idf_hal::gpio::{AnyIOPin, InputPin, OutputPin};
use esp_idf_hal::i2s::{
    config::{DataBitWidth, StdConfig},
    I2s, I2sDriver, I2sTx,
};
use fp_core::OscillatorRt;
use minimp3_sys::{mp3d_sample_t, mp3dec_decode_frame, mp3dec_frame_info_t, mp3dec_init, mp3dec_t};
use std::fs::File;
use std::io::{BufReader, Read};

use crate::state::STATE;
use crate::storage;

pub const SAMPLE_RATE: u32    = 44_100;
pub const BUFFER_FRAMES: usize = 512;

// How much compressed MP3 data to keep in the read-ahead buffer.
// Max MP3 frame at 320 kbps/44.1 kHz is ~1044 bytes; 16 KB is plenty.
const MP3_BUF_CAP:   usize = 16 * 1024;
// Refill when available compressed data drops below this.
const MP3_REFILL_AT: usize =  4 * 1024;
// BufReader capacity: absorbs SPIFFS page-boundary partial reads so
// our MP3 buffer can fill completely in one refill() call.
const FS_BUF_CAP:    usize = 32 * 1024;

// ── Oscillator path ───────────────────────────────────────────────────────────

fn fill_oscillator(osc: &mut OscillatorRt, out: &mut Vec<u8>) {
    out.clear();
    let freq    = STATE.current_freq();
    let playing = STATE.is_playing();
    let wave    = STATE.waveform();
    let filter  = STATE.filter();
    let volume  = STATE.volume() as f64 / 100.0;

    for _ in 0..BUFFER_FRAMES {
        let sample = if playing {
            (osc.tick(wave, filter, freq, SAMPLE_RATE as f64)
                .clamp(-1.0, 1.0) * volume * i32::MAX as f64) as i32
        } else {
            0i32
        };
        let b = sample.to_le_bytes();
        out.extend_from_slice(&b); // L
        out.extend_from_slice(&b); // R
    }
}

// ── MP3 path — uses minimp3_sys (raw C bindings) with a simple Vec buffer ─────
//
// We avoid the higher-level `minimp3` crate because its Rust wrapper depends on
// `slice-deque`, which uses SysV IPC symbols (`shmat`, `shmget`, …) that do not
// exist in ESP-IDF's libc implementation.

struct Mp3Player {
    file:     BufReader<File>,
    dec:      mp3dec_t,          // minimp3 decoder state (~4 KB)
    buf:      Vec<u8>,           // compressed MP3 read-ahead data
    buf_pos:  usize,             // start of unconsumed data in `buf`
    eof:      bool,

    // Last decoded PCM frame (MINIMP3_MAX_SAMPLES_PER_FRAME = 1152*2 = 2304 samples)
    pcm:      [mp3d_sample_t; 2304],
    pcm_count: usize,            // samples per channel from the last frame
    pcm_pos:   usize,            // stereo pairs consumed from this frame
    channels:  usize,

    playing_idx: usize,
}

impl Mp3Player {
    fn open(idx: usize) -> Option<Box<Self>> {
        let name = storage::audio_file_name(idx);
        if name.is_empty() { return None; }
        let raw = File::open(storage::vfs_path(&name)).ok()?;
        let file = BufReader::with_capacity(FS_BUF_CAP, raw);
        let mut p = Box::new(Self {
            file,
            dec: unsafe { core::mem::zeroed() },
            buf: Vec::with_capacity(MP3_BUF_CAP),
            buf_pos: 0,
            eof: false,
            pcm: [0; 2304],
            pcm_count: 0,
            pcm_pos: 0,
            channels: 2,
            playing_idx: idx,
        });
        unsafe { mp3dec_init(&mut p.dec) };
        Some(p)
    }

    /// Refill the compressed-data buffer from disk when it runs low.
    fn refill(&mut self) {
        let available = self.buf.len() - self.buf_pos;
        if available >= MP3_REFILL_AT || self.eof { return; }

        // Compact: move unconsumed data to the front of the Vec.
        if self.buf_pos > 0 {
            self.buf.copy_within(self.buf_pos.., 0);
            self.buf.truncate(available);
            self.buf_pos = 0;
        }

        // Fill up to MP3_BUF_CAP, looping to handle partial reads.
        // SPIFFS may satisfy a read() with less than requested (page-aligned
        // chunks); BufReader batches those at the FS level, but we loop here
        // too so the MP3 decode buffer is always as full as possible.
        let old_len = self.buf.len();
        if old_len >= MP3_BUF_CAP { return; }
        self.buf.resize(MP3_BUF_CAP, 0);
        let mut filled = 0usize;
        while old_len + filled < MP3_BUF_CAP {
            match self.file.read(&mut self.buf[old_len + filled..]) {
                Ok(0)  => { self.eof = true; break; }
                Ok(n)  => { filled += n; }
                Err(_) => { self.eof = true; break; }
            }
        }
        self.buf.truncate(old_len + filled);
    }

    /// Decode the next MP3 frame into `self.pcm`.
    /// Returns `false` on EOF / unrecoverable error.
    fn next_frame(&mut self) -> bool {
        loop {
            self.refill();
            let available = self.buf.len() - self.buf_pos;
            if available == 0 { return false; }

            let mut info = unsafe { core::mem::zeroed::<mp3dec_frame_info_t>() };
            let samples = unsafe {
                mp3dec_decode_frame(
                    &mut self.dec,
                    self.buf[self.buf_pos..].as_ptr(),
                    available as i32,
                    self.pcm.as_mut_ptr(),
                    &mut info,
                )
            };

            let consumed = info.frame_bytes as usize;
            if consumed > 0 {
                self.buf_pos += consumed;
            } else {
                // No frame sync — skip one byte and retry.
                self.buf_pos += 1;
                continue;
            }

            if samples > 0 {
                self.pcm_count = samples as usize;
                self.channels  = info.channels as usize;
                self.pcm_pos   = 0;
                return true;
            }
            // samples == 0: consumed an ID3/Xing tag — no audio, try next frame.
        }
    }

    /// Fill `out` (BUFFER_FRAMES × 2 interleaved stereo i16) from the decoder.
    /// Returns `false` on EOF; any unfilled portion is zeroed.
    fn fill(&mut self, out: &mut [i16]) -> bool {
        let mut written = 0;
        while written < out.len() {
            if self.pcm_pos < self.pcm_count {
                if self.channels == 1 {
                    let s = self.pcm[self.pcm_pos];
                    out[written]     = s;
                    out[written + 1] = s;
                } else {
                    out[written]     = self.pcm[self.pcm_pos * 2];
                    out[written + 1] = self.pcm[self.pcm_pos * 2 + 1];
                }
                written  += 2;
                self.pcm_pos += 1;
                continue;
            }

            if !self.next_frame() {
                for s in &mut out[written..] { *s = 0; }
                return false;
            }
        }
        true
    }
}

// ── Audio task ────────────────────────────────────────────────────────────────

pub fn audio_task(
    i2s:  impl I2s + 'static,
    bck:  impl InputPin + OutputPin + 'static,
    ws:   impl InputPin + OutputPin + 'static,
    dout: impl OutputPin + 'static,
) -> Result<()> {
    let std_cfg = StdConfig::philips(SAMPLE_RATE, DataBitWidth::Bits32);
    let mut driver = I2sDriver::<I2sTx>::new_std_tx(i2s, &std_cfg, bck, dout, None::<AnyIOPin>, ws)?;
    driver.tx_enable()?;

    let mut osc = OscillatorRt::new();
    // Mp3Player is Boxed: the struct contains a ~4 KB mp3dec_t and a 16 KB
    // read buffer — keep them off the audio thread's stack.
    let mut mp3: Option<Box<Mp3Player>> = None;

    let mut pcm     = vec![0i16; BUFFER_FRAMES * 2]; // staging: interleaved stereo i16
    let mut i2s_buf = Vec::<u8>::with_capacity(BUFFER_FRAMES * 2 * 4);
    let timeout = esp_idf_hal::delay::BLOCK;

    loop {
        let mp3_active = STATE.is_mp3_mode() && STATE.is_playing();

        if mp3_active {
            let idx = STATE.mp3_selected();

            // Open (or reopen if the selected track changed while playing).
            if mp3.as_ref().map_or(true, |p| p.playing_idx != idx) {
                mp3 = Mp3Player::open(idx);
                STATE.reset_playback_timer();
                if mp3.is_none() { STATE.stop_playing(); }
            }

            if let Some(ref mut player) = mp3 {
                let vol = STATE.volume() as f32 / 100.0;
                if player.fill(&mut pcm) {
                    i2s_buf.clear();
                    for chunk in pcm.chunks_exact(2) {
                        let l = ((chunk[0] as f32 * vol) as i32) << 16;
                        let r = ((chunk[1] as f32 * vol) as i32) << 16;
                        i2s_buf.extend_from_slice(&l.to_le_bytes());
                        i2s_buf.extend_from_slice(&r.to_le_bytes());
                    }
                } else {
                    STATE.stop_playing();
                    mp3 = None;
                    i2s_buf.clear();
                    i2s_buf.resize(BUFFER_FRAMES * 2 * 4, 0);
                }
            } else {
                i2s_buf.clear();
                i2s_buf.resize(BUFFER_FRAMES * 2 * 4, 0);
            }
        } else {
            mp3 = None; // drop decoder when not in MP3 playback mode
            fill_oscillator(&mut osc, &mut i2s_buf);
        }

        driver.write(&i2s_buf, timeout)?;

        // Advance the playback timer only while actively decoding MP3.
        if mp3_active && mp3.is_some() {
            STATE.advance_playback(BUFFER_FRAMES as u32);
        }
    }
}
