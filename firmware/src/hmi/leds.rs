//! WS2811 LED driver using ESP-IDF RMT peripheral.
//!
//! Two channels: ring (60 LEDs, pin 38, RGB) and button strip (8 LEDs, pin 42, GRB).

use esp_idf_sys::*;
use nanod_math::led::types::Rgb;

/// WS2811 timing at 10MHz RMT resolution clock.
/// T0H = 300ns (3 ticks), T0L = 900ns (9 ticks)
/// T1H = 600ns (6 ticks), T1L = 600ns (6 ticks)
const RMT_RESOLUTION_HZ: u32 = 10_000_000;

fn ws2811_bit0() -> rmt_symbol_word_t {
    unsafe {
        let mut sym: rmt_symbol_word_t = core::mem::zeroed();
        // T0H: 3 ticks high, T0L: 9 ticks low
        sym.__bindgen_anon_1.set_duration0(3);
        sym.__bindgen_anon_1.set_level0(1);
        sym.__bindgen_anon_1.set_duration1(9);
        sym.__bindgen_anon_1.set_level1(0);
        sym
    }
}

fn ws2811_bit1() -> rmt_symbol_word_t {
    unsafe {
        let mut sym: rmt_symbol_word_t = core::mem::zeroed();
        // T1H: 6 ticks high, T1L: 6 ticks low
        sym.__bindgen_anon_1.set_duration0(6);
        sym.__bindgen_anon_1.set_level0(1);
        sym.__bindgen_anon_1.set_duration1(6);
        sym.__bindgen_anon_1.set_level1(0);
        sym
    }
}

pub struct Ws2811Driver {
    ring_channel: rmt_channel_handle_t,
    button_channel: rmt_channel_handle_t,
    ring_encoder: rmt_encoder_handle_t,
    button_encoder: rmt_encoder_handle_t,
}

impl Ws2811Driver {
    /// Initialize two RMT TX channels for the LED strips.
    pub fn new(ring_pin: i32, button_pin: i32) -> Result<Self, EspError> {
        let ring_channel = Self::create_channel(ring_pin)?;
        let button_channel = Self::create_channel(button_pin)?;
        let ring_encoder = Self::create_encoder()?;
        let button_encoder = Self::create_encoder()?;

        unsafe {
            esp!(rmt_enable(ring_channel))?;
            esp!(rmt_enable(button_channel))?;
        }

        Ok(Self {
            ring_channel,
            button_channel,
            ring_encoder,
            button_encoder,
        })
    }

    fn create_channel(gpio_num: i32) -> Result<rmt_channel_handle_t, EspError> {
        let mut channel: rmt_channel_handle_t = core::ptr::null_mut();
        let mut config: rmt_tx_channel_config_t = unsafe { core::mem::zeroed() };
        config.gpio_num = gpio_num;
        config.clk_src = soc_periph_rmt_clk_src_t_RMT_CLK_SRC_DEFAULT;
        config.resolution_hz = RMT_RESOLUTION_HZ;
        config.mem_block_symbols = 64; // minimum block size
        config.trans_queue_depth = 1;

        unsafe { esp!(rmt_new_tx_channel(&config, &mut channel))? };
        Ok(channel)
    }

    fn create_encoder() -> Result<rmt_encoder_handle_t, EspError> {
        let mut encoder: rmt_encoder_handle_t = core::ptr::null_mut();
        let config = rmt_bytes_encoder_config_t {
            bit0: ws2811_bit0(),
            bit1: ws2811_bit1(),
            flags: unsafe { core::mem::zeroed() },
        };

        unsafe { esp!(rmt_new_bytes_encoder(&config, &mut encoder))? };
        Ok(encoder)
    }

    /// Transmit RGB data to the ring strip (60 LEDs, RGB byte order).
    pub fn show_ring(&mut self, pixels: &[Rgb; 60]) -> Result<(), EspError> {
        let mut buf = [0u8; 60 * 3];
        for (i, px) in pixels.iter().enumerate() {
            buf[i * 3] = px.r;
            buf[i * 3 + 1] = px.g;
            buf[i * 3 + 2] = px.b;
        }
        self.transmit(self.ring_channel, self.ring_encoder, &buf)
    }

    /// Transmit RGB data to the button strip (8 LEDs, GRB byte order).
    pub fn show_buttons(&mut self, pixels: &[Rgb; 8]) -> Result<(), EspError> {
        let mut buf = [0u8; 8 * 3];
        for (i, px) in pixels.iter().enumerate() {
            // GRB order for button strip
            buf[i * 3] = px.g;
            buf[i * 3 + 1] = px.r;
            buf[i * 3 + 2] = px.b;
        }
        self.transmit(self.button_channel, self.button_encoder, &buf)
    }

    fn transmit(
        &self,
        channel: rmt_channel_handle_t,
        encoder: rmt_encoder_handle_t,
        data: &[u8],
    ) -> Result<(), EspError> {
        let tx_config = rmt_transmit_config_t {
            loop_count: 0, // no loop
            flags: unsafe { core::mem::zeroed() },
        };

        unsafe {
            esp!(rmt_transmit(
                channel,
                encoder,
                data.as_ptr() as *const _,
                data.len(),
                &tx_config,
            ))?;

            // Wait for transmission to complete (reset signal needs ~280us)
            esp!(rmt_tx_wait_all_done(channel, 100))?; // 100ms timeout
        }

        Ok(())
    }
}

impl Drop for Ws2811Driver {
    fn drop(&mut self) {
        unsafe {
            rmt_disable(self.ring_channel);
            rmt_disable(self.button_channel);
            rmt_del_channel(self.ring_channel);
            rmt_del_channel(self.button_channel);
            rmt_del_encoder(self.ring_encoder);
            rmt_del_encoder(self.button_encoder);
        }
    }
}
