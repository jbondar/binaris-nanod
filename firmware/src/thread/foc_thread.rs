use esp_idf_hal::peripherals::Peripherals;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_sys::*;

use crate::ipc::{AngleSnapshot, FocCommand, FocContext};
use crate::motor::calibration;
use crate::motor::encoder::Mt6701Encoder;
use crate::haptic::profile::Direction;
use crate::pins;

const FOC_STACK_SIZE: usize = 16384;
const FOC_PRIORITY: u32 = 1;
const FOC_CORE: i32 = 1;

/// How often to publish angle snapshots (microseconds).
const ANGLE_PUBLISH_INTERVAL_US: u64 = 10_000; // 10ms

/// Spawn the FOC control thread pinned to Core 1.
pub fn spawn_foc_thread(ctx: FocContext) {
    let ctx_ptr = Box::into_raw(Box::new(ctx)) as *mut core::ffi::c_void;
    unsafe {
        let mut handle: TaskHandle_t = core::ptr::null_mut();
        xTaskCreatePinnedToCore(
            Some(foc_task),
            b"foc\0".as_ptr() as *const _,
            FOC_STACK_SIZE as u32,
            ctx_ptr,
            FOC_PRIORITY,
            &mut handle,
            FOC_CORE,
        );
    }
}

unsafe extern "C" fn foc_task(arg: *mut core::ffi::c_void) {
    let ctx = unsafe { *Box::from_raw(arg as *mut FocContext) };
    if let Err(e) = foc_task_inner(ctx) {
        log::error!("FOC task failed: {:?}", e);
    }
    loop {
        vTaskDelay(1000);
    }
}

fn foc_task_inner(ctx: FocContext) -> Result<(), EspError> {
    log::info!("FOC thread starting");

    let _peripherals = unsafe { Peripherals::steal() };
    let nvs_partition = EspDefaultNvsPartition::take()?;

    // --- Encoder (Rust SPI, polling mode) ---
    let mut encoder = Mt6701Encoder::new(
        spi_host_device_t_SPI2_HOST,
        pins::MAG_CS,
        pins::MAG_CLK,
        pins::MAG_DO,
        -1, // no MOSI
    )?;

    // --- Initialize C++ SimpleFOC motor ---
    let pwm_pins = [pins::MOTOR_IN_U, pins::MOTOR_IN_V, pins::MOTOR_IN_W];
    let enable_pins = [pins::MOTOR_EN_U, pins::MOTOR_EN_V, pins::MOTOR_EN_W];

    let ret = unsafe {
        nanod_motor_init(
            pwm_pins.as_ptr(),
            enable_pins.as_ptr(),
            5.0,   // voltage_supply
            5.0,   // voltage_limit
            5.3,   // phase_resistance
            1.22,  // current_limit
        )
    };
    if ret != 0 {
        log::error!("nanod_motor_init failed: {ret}");
        return Err(EspError::from_infallible::<{ ESP_FAIL }>());
    }
    log::info!("SimpleFOC motor initialized");

    // --- Load or run calibration ---
    let cal = calibration::load_calibration(nvs_partition.clone())?;
    if cal.direction != Direction::Unknown {
        let dir: i32 = match cal.direction {
            Direction::Cw => 1,
            Direction::Ccw => 2, // SimpleFOC Direction::CCW = 2
            Direction::Unknown => 0,
        };
        unsafe { nanod_motor_set_calibration(dir, cal.zero_angle) };
        log::info!(
            "Loaded calibration: dir={}, zero={:.4}",
            dir,
            cal.zero_angle
        );
    } else {
        log::warn!("No calibration — running SimpleFOC auto-calibration");
        // Feed encoder readings during calibration (SimpleFOC reads sensor internally)
        // Need to do a first sensor update
        let angle = encoder.read_angle()?;
        unsafe { nanod_motor_set_encoder_angle(angle) };

        let result = unsafe { nanod_motor_calibrate() };
        if result != 0 {
            // Save calibration to NVS
            let mut dir: i32 = 0;
            let mut zero: f32 = 0.0;
            unsafe { nanod_motor_get_calibration(&mut dir, &mut zero) };

            let direction = match dir {
                1 => Direction::Cw,
                2 => Direction::Ccw,
                _ => Direction::Unknown,
            };
            let cal_data = crate::haptic::profile::MotorCalibration {
                direction,
                zero_angle: zero,
            };
            calibration::store_calibration(nvs_partition.clone(), &cal_data)?;
            log::info!("Calibration complete: dir={dir}, zero={zero:.4}");
        } else {
            log::error!("SimpleFOC calibration failed");
        }
    }

    // --- Load default haptic profile ---
    unsafe {
        nanod_motor_load_profile(
            0,    // mode: REGULAR
            0,    // start_pos
            120,  // end_pos
            20,   // detent_count
            1,    // vernier
            0,    // kx_force
            0.0,  // output_ramp (0 = no ramp)
            3.0,  // detent_strength
        );
    }
    log::info!("Loaded default haptic profile (20 detents, 0-120)");

    let mut last_publish_us: u64 = 0;
    let mut loop_count: u32 = 0;
    let mut rate_measure_us: u64 = 0;

    log::info!("FOC thread entering control loop");

    // --- Main control loop ---
    loop {
        let now_us = unsafe { esp_timer_get_time() } as u64;

        // Check for incoming commands (every 100 loops)
        if loop_count % 100 == 0 {
            if let Ok(cmd) = ctx.cmd_rx.try_recv() {
                match cmd {
                    FocCommand::UpdateHaptic(profile) => {
                        let mode = match profile.mode {
                            crate::haptic::profile::HapticMode::Regular => 0u8,
                            crate::haptic::profile::HapticMode::Vernier => 1,
                            crate::haptic::profile::HapticMode::Viscose => 2,
                            crate::haptic::profile::HapticMode::Spring => 3,
                        };
                        unsafe {
                            nanod_motor_load_profile(
                                mode,
                                profile.start_pos,
                                profile.end_pos,
                                profile.detent_count,
                                profile.vernier,
                                profile.kx_force as u8,
                                profile.output_ramp,
                                profile.detent_strength,
                            );
                        }
                    }
                    FocCommand::Recalibrate => {
                        unsafe { nanod_motor_recalibrate() };
                    }
                }
            }
        }

        // Read encoder and feed to SimpleFOC
        let angle = encoder.read_angle()?;
        unsafe { nanod_motor_set_encoder_angle(angle) };

        // Run SimpleFOC + haptic loop (loopFOC + find_detent + haptic_target + move)
        unsafe { nanod_motor_loop() };

        // Publish angle snapshot to HMI + display
        if now_us.wrapping_sub(last_publish_us) >= ANGLE_PUBLISH_INTERVAL_US {
            last_publish_us = now_us;
            let pos = unsafe { nanod_motor_get_position() };
            let shaft = unsafe { nanod_motor_get_shaft_angle() };
            let _ = ctx.angle_tx.try_send(AngleSnapshot {
                shaft_angle: shaft,
                current_pos: pos,
            });
            let _ = ctx.display_tx.try_send(AngleSnapshot {
                shaft_angle: shaft,
                current_pos: pos,
            });
        }

        // Rate measurement
        loop_count += 1;
        if now_us.wrapping_sub(rate_measure_us) >= 5_000_000 {
            let rate = loop_count as f32 / 5.0;
            unsafe {
                static mut LOG_COUNT: u32 = 0;
                LOG_COUNT += 1;
                if LOG_COUNT <= 3 {
                    log::info!("FOC loop: {:.0} Hz", rate);
                }
            }
            loop_count = 0;
            rate_measure_us = now_us;
        }
    }
}
