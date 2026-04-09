use esp_idf_hal::gpio::{Output, PinDriver};
use esp_idf_sys::*;

use super::foc::PhaseDuty;

/// MCPWM-based 3-phase motor driver.
///
/// Uses ESP-IDF MCPWM peripheral via FFI for precise PWM generation,
/// with standard GPIO for the enable pins.
pub struct ThreePhaseDriver<'a> {
    timer: mcpwm_timer_handle_t,
    comparators: [mcpwm_cmpr_handle_t; 3],
    enable_pins: [PinDriver<'a, Output>; 3],
    period_ticks: u32,
}

impl<'a> ThreePhaseDriver<'a> {
    /// Initialize MCPWM timer, operators, comparators, and generators.
    pub fn new(
        group_id: i32,
        pwm_pins: [i32; 3],
        enable_pins: [PinDriver<'a, Output>; 3],
        frequency_hz: u32,
    ) -> Result<Self, EspError> {
        let period_ticks = 1_000_000 / frequency_hz;

        let mut timer_config: mcpwm_timer_config_t = unsafe { core::mem::zeroed() };
        timer_config.group_id = group_id;
        timer_config.clk_src = soc_periph_mcpwm_timer_clk_src_t_MCPWM_TIMER_CLK_SRC_DEFAULT;
        timer_config.resolution_hz = 1_000_000;
        timer_config.count_mode = mcpwm_timer_count_mode_t_MCPWM_TIMER_COUNT_MODE_UP;
        timer_config.period_ticks = period_ticks;

        let mut timer: mcpwm_timer_handle_t = core::ptr::null_mut();
        esp!(unsafe { mcpwm_new_timer(&timer_config, &mut timer) })?;

        let mut comparators = [core::ptr::null_mut(); 3];

        for (i, &pwm_pin) in pwm_pins.iter().enumerate() {
            let mut oper_config: mcpwm_operator_config_t = unsafe { core::mem::zeroed() };
            oper_config.group_id = group_id;

            let mut oper: mcpwm_oper_handle_t = core::ptr::null_mut();
            esp!(unsafe { mcpwm_new_operator(&oper_config, &mut oper) })?;
            esp!(unsafe { mcpwm_operator_connect_timer(oper, timer) })?;

            let cmpr_config: mcpwm_comparator_config_t = unsafe { core::mem::zeroed() };
            let mut cmpr: mcpwm_cmpr_handle_t = core::ptr::null_mut();
            esp!(unsafe { mcpwm_new_comparator(oper, &cmpr_config, &mut cmpr) })?;
            comparators[i] = cmpr;

            let mut gen_config: mcpwm_generator_config_t = unsafe { core::mem::zeroed() };
            gen_config.gen_gpio_num = pwm_pin;

            let mut gen: mcpwm_gen_handle_t = core::ptr::null_mut();
            esp!(unsafe { mcpwm_new_generator(oper, &gen_config, &mut gen) })?;

            esp!(unsafe {
                mcpwm_generator_set_action_on_timer_event(
                    gen,
                    mcpwm_gen_timer_event_action_t {
                        direction: mcpwm_timer_direction_t_MCPWM_TIMER_DIRECTION_UP,
                        event: mcpwm_timer_event_t_MCPWM_TIMER_EVENT_EMPTY,
                        action: mcpwm_generator_action_t_MCPWM_GEN_ACTION_HIGH,
                    },
                )
            })?;
            esp!(unsafe {
                mcpwm_generator_set_action_on_compare_event(
                    gen,
                    mcpwm_gen_compare_event_action_t {
                        direction: mcpwm_timer_direction_t_MCPWM_TIMER_DIRECTION_UP,
                        comparator: cmpr,
                        action: mcpwm_generator_action_t_MCPWM_GEN_ACTION_LOW,
                    },
                )
            })?;
        }

        esp!(unsafe { mcpwm_timer_enable(timer) })?;
        esp!(unsafe {
            mcpwm_timer_start_stop(timer, mcpwm_timer_start_stop_cmd_t_MCPWM_TIMER_START_NO_STOP)
        })?;

        Ok(Self {
            timer,
            comparators,
            enable_pins,
            period_ticks,
        })
    }

    /// Set three-phase duty cycles (0.0 to 1.0 each).
    pub fn set_pwm(&mut self, duty: PhaseDuty) -> Result<(), EspError> {
        let duties = [duty.a, duty.b, duty.c];
        for (i, &d) in duties.iter().enumerate() {
            let ticks = (d * self.period_ticks as f32) as u32;
            esp!(unsafe { mcpwm_comparator_set_compare_value(self.comparators[i], ticks) })?;
        }
        Ok(())
    }

    /// Enable all three half-bridge outputs.
    pub fn enable(&mut self) -> Result<(), EspError> {
        for pin in &mut self.enable_pins {
            pin.set_high()?;
        }
        Ok(())
    }

    /// Disable all three half-bridge outputs.
    pub fn disable(&mut self) -> Result<(), EspError> {
        for pin in &mut self.enable_pins {
            pin.set_low()?;
        }
        Ok(())
    }
}

impl<'a> Drop for ThreePhaseDriver<'a> {
    fn drop(&mut self) {
        let _ = self.disable();
        unsafe {
            mcpwm_timer_start_stop(
                self.timer,
                mcpwm_timer_start_stop_cmd_t_MCPWM_TIMER_STOP_EMPTY,
            );
            mcpwm_timer_disable(self.timer);
            mcpwm_del_timer(self.timer);
        }
    }
}
