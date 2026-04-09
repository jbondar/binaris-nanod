/// NanoD hardware pin assignments (from nanofoc_d.h).
///
/// These constants are used to configure peripherals at init time.
/// The actual typed GPIO pins are obtained from the ESP Peripherals struct.

// Motor driver — 3-phase half-bridge
pub const MOTOR_EN_U: i32 = 33;
pub const MOTOR_EN_V: i32 = 48;
pub const MOTOR_EN_W: i32 = 36;
pub const MOTOR_IN_U: i32 = 34;
pub const MOTOR_IN_V: i32 = 35;
pub const MOTOR_IN_W: i32 = 37;

// Magnetic encoder (MT6701 SSI via HSPI)
pub const MAG_CLK: i32 = 18;
pub const MAG_DO: i32 = 21;
pub const MAG_CS: i32 = 17;

// I2C bus
pub const I2C_SDA: i32 = 12;
pub const I2C_SCL: i32 = 13;

// LEDs
pub const LED_A: i32 = 38;
pub const LED_B: i32 = 42;

// Buttons
pub const BTN_A: i32 = 41;
pub const BTN_B: i32 = 40;
pub const BTN_C: i32 = 45;
pub const BTN_D: i32 = 46;

// I2S audio
pub const I2S_DOUT: i32 = 9;
pub const I2S_BCLK: i32 = 10;
pub const I2S_LRC: i32 = 11;

// Display SPI
pub const DISP_MOSI: i32 = 4;
pub const DISP_SCLK: i32 = 3;
pub const DISP_CS: i32 = 6;
pub const DISP_DC: i32 = 7;
pub const DISP_RST: i32 = 2;

// Serial2
pub const SERIAL2_RX: i32 = 44;
pub const SERIAL2_TX: i32 = 43;
