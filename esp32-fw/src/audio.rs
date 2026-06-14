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

use crate::state::STATE;

pub const SAMPLE_RATE: u32   = 44_100;
pub const BUFFER_FRAMES: usize = 512; // samples per channel per fill

/// Fill one stereo I2S buffer from the oscillator engine.
/// Returns a Vec<u8> ready for i2s_driver.write().
fn fill_buffer(osc: &mut OscillatorRt) -> Vec<u8> {
    // 2 channels × 4 bytes (i32) × BUFFER_FRAMES
    let mut buf: Vec<u8> = Vec::with_capacity(BUFFER_FRAMES * 2 * 4);

    let freq    = STATE.current_freq();
    let playing = STATE.is_playing();
    let wave    = STATE.waveform();
    let filter  = STATE.filter();

    for _ in 0..BUFFER_FRAMES {
        let sample = if playing {
            (osc.tick(wave, filter, freq, SAMPLE_RATE as f64).clamp(-1.0, 1.0)
                * i32::MAX as f64) as i32
        } else {
            0i32
        };

        // I2S expects little-endian 32-bit words, L then R.
        let bytes = sample.to_le_bytes();
        buf.extend_from_slice(&bytes); // left
        buf.extend_from_slice(&bytes); // right (mono — same signal both channels)
    }

    buf
}

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
    let timeout = esp_idf_hal::delay::BLOCK;

    loop {
        let buf = fill_buffer(&mut osc);
        driver.write(&buf, timeout)?;
    }
}
