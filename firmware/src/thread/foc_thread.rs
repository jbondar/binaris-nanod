use esp_idf_hal::gpio::{AnyInputPin, PinDriver};
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_hal::spi::{SpiDriver, SpiDriverConfig};
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_sys::*;

use crate::haptic::profile::Direction;
use crate::haptic::state::HapticController;
use crate::motor::calibration;
use crate::motor::driver::ThreePhaseDriver;
use crate::motor::encoder::Mt6701Encoder;
use crate::motor::foc::{FocState, MotorConfig};
use crate::pins;

const FOC_STACK_SIZE: usize = 8192;
const FOC_PRIORITY: u32 = 1;
const FOC_CORE: i32 = 1;
const PWM_FREQUENCY_HZ: u32 = 25_000;

/// Spawn the FOC control thread pinned to Core 1.
pub fn spawn_foc_thread() {
    unsafe {
        let mut handle: TaskHandle_t = core::ptr::null_mut();
        xTaskCreatePinnedToCore(
            Some(foc_task),
            b"foc\0".as_ptr() as *const _,
            FOC_STACK_SIZE as u32,
            core::ptr::null_mut(),
            FOC_PRIORITY,
            &mut handle,
            FOC_CORE,
        );
    }
}

unsafe extern "C" fn foc_task(_arg: *mut core::ffi::c_void) {
    if let Err(e) = foc_task_inner() {
        log::error!("FOC task failed: {:?}", e);
    }
    loop {
        vTaskDelay(1000);
    }
}

fn foc_task_inner() -> Result<(), EspError> {
    log::info!("FOC thread starting");

    let peripherals = unsafe { Peripherals::steal() };
    let nvs_partition = EspDefaultNvsPartition::take()?;

    // --- SPI bus for encoder ---
    let spi_driver = SpiDriver::new(
        peripherals.spi2,
        peripherals.pins.gpio18, // CLK
        peripherals.pins.gpio21, // MOSI (DO from sensor)
        None::<AnyInputPin>,     // No MISO needed for SSI read
        &SpiDriverConfig::new(),
    )?;

    let mut encoder = Mt6701Encoder::new(
        &spi_driver,
        peripherals.pins.gpio17.into(), // CS
    )?;

    // --- Motor driver (MCPWM) ---
    let en_u = PinDriver::output(peripherals.pins.gpio33)?;
    let en_v = PinDriver::output(peripherals.pins.gpio48)?;
    let en_w = PinDriver::output(peripherals.pins.gpio36)?;

    let mut driver = ThreePhaseDriver::new(
        0,
        [pins::MOTOR_IN_U, pins::MOTOR_IN_V, pins::MOTOR_IN_W],
        [en_u, en_v, en_w],
        PWM_FREQUENCY_HZ,
    )?;
    driver.enable()?;

    // --- FOC state ---
    let mut foc = FocState::new(MotorConfig::default());

    // --- Load calibration ---
    let cal = calibration::load_calibration(nvs_partition.clone())?;
    if cal.direction != Direction::Unknown {
        foc.sensor_direction = match cal.direction {
            Direction::Cw => 1,
            Direction::Ccw => -1,
            Direction::Unknown => 1,
        };
        foc.zero_electrical_angle = cal.zero_angle;
        log::info!(
            "Loaded calibration: dir={}, zero={}",
            foc.sensor_direction,
            foc.zero_electrical_angle
        );
    } else {
        log::warn!("No calibration found — motor will need calibration on first run");
    }

    // --- Haptic controller ---
    let mut haptic = HapticController::new();
    haptic.sensor_direction = cal.direction;

    log::info!("FOC thread initialized, entering control loop");

    // --- Main control loop ---
    loop {
        let now_us = unsafe { esp_timer_get_time() } as u64;

        let angle = encoder.read_angle()?;
        foc.update_sensor(angle, now_us);

        let output = haptic.haptic_loop(foc.shaft_angle, foc.shaft_velocity, now_us);

        if output.run_foc {
            let duty = foc.compute_torque(output.pid_error);
            driver.set_pwm(duty)?;
        } else {
            loop {
                let settle_now = unsafe { esp_timer_get_time() } as u64;
                let settle_angle = encoder.read_angle()?;
                foc.update_sensor(settle_angle, settle_now);

                let (error, should_break) =
                    haptic.bounds_settle_error(foc.shaft_angle, foc.shaft_velocity);

                let duty = foc.compute_torque(haptic.pid.call(error, settle_now));
                driver.set_pwm(duty)?;

                if should_break {
                    break;
                }
            }
        }
    }
}
