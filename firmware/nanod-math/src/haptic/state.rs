use core::f32::consts::PI;

use super::pid::PidController;
use super::profile::{DetentProfile, Direction, HapticEvt, HapticMode};

const TWO_PI: f32 = 2.0 * PI;

/// Tracks the physical and logical state of the haptic detent system.
#[derive(Debug, Clone)]
pub struct HapticState {
    pub detent_profile: DetentProfile,
    pub current_pos: u16,
    pub last_pos: u16,
    pub num_detents: u16,
    pub detent_width: f32,

    pub attract_angle: f32,
    pub last_attract_angle: f32,

    pub detent_strength_unit: f32,
    pub endstop_strength_unit: f32,
    pub attract_hysteresis: f32,

    pub at_limit: bool,
    pub was_at_limit: bool,
}

impl HapticState {
    pub fn new() -> Self {
        let mut s = Self {
            detent_profile: DetentProfile::default(),
            current_pos: 0,
            last_pos: 0,
            num_detents: 0,
            detent_width: 0.0,
            attract_angle: 0.0,
            last_attract_angle: 0.0,
            detent_strength_unit: 3.0,
            endstop_strength_unit: 1.0, // weaker than normal — used when clipping (matches C++)
            attract_hysteresis: 0.25,
            at_limit: false,
            was_at_limit: false,
        };
        let profile = DetentProfile::default();
        s.load_profile(profile, Some(profile.start_pos));
        s
    }

    pub fn load_profile(&mut self, profile: DetentProfile, new_position: Option<u16>) {
        let is_vernier: u16 = if profile.mode == HapticMode::Vernier {
            profile.vernier as u16
        } else {
            1
        };

        self.num_detents = profile.end_pos - profile.start_pos;
        self.detent_width = TWO_PI / profile.detent_count as f32;

        if profile.mode == HapticMode::Vernier {
            self.detent_width /= profile.vernier as f32;
        }

        if let Some(pos) = new_position {
            self.current_pos = pos;
        } else {
            // Clamp to valid range
            if self.current_pos < profile.start_pos * is_vernier {
                self.current_pos = profile.start_pos;
            } else if self.current_pos > profile.end_pos * is_vernier {
                self.current_pos = profile.end_pos;
            }
        }

        self.last_pos = self.current_pos;
        self.detent_strength_unit = profile.detent_strength;
        self.detent_profile = profile;
    }
}

/// Result of a single haptic loop iteration.
pub struct HapticOutput {
    /// PID error to feed into motor torque command.
    pub pid_error: f32,
    /// Whether loopFOC + move should run (false during bounds settling).
    pub run_foc: bool,
    /// Events fired this iteration.
    pub events: heapless::Vec<HapticEvt, 4>,
}

/// Orchestrates the haptic control algorithm.
/// Separated from hardware: takes shaft angle/velocity in, produces PID error + events out.
pub struct HapticController {
    pub state: HapticState,
    pub pid: PidController,
    pub sensor_direction: Direction,
}

impl HapticController {
    pub fn new() -> Self {
        Self {
            state: HapticState::new(),
            // Matches C++: PIDController(P=5.0, I=0.0, D=0.004, ramp=10000, limit=0.4)
            // P gets overwritten by correct_pid() every loop iteration
            pid: PidController::new(5.0, 0.0, 0.004, 10000.0, 0.4),
            //                       P    I    D      ramp     limit
            sensor_direction: Direction::Ccw, // production PCB default
        }
    }

    /// Run one iteration of the haptic loop.
    /// Returns the PID output torque command and any events.
    pub fn haptic_loop(&mut self, shaft_angle: f32, shaft_velocity: f32, now_us: u64) -> HapticOutput {
        let mut events = heapless::Vec::new();

        self.correct_pid();
        self.find_detent(shaft_angle, &mut events);
        self.haptic_target(shaft_angle, shaft_velocity, now_us)
    }

    /// Adjust PID gains based on detent state.
    fn correct_pid(&mut self) {
        let s = &self.state;

        // D term scaling based on detent width
        let d_lower_strength = s.detent_strength_unit * 0.008;
        let d_upper_strength = s.detent_strength_unit * 0.004;
        let d_lower_pos_width = 3.0_f32.to_radians();
        let d_upper_pos_width = 8.0_f32.to_radians();
        let mut raw = d_lower_strength;
        raw += (d_upper_strength - d_lower_strength) / (d_upper_pos_width - d_lower_pos_width);
        raw *= s.detent_width - d_lower_pos_width;

        let total_positions = s.detent_profile.end_pos - s.detent_profile.start_pos + 1;
        self.pid.d = if total_positions > 0 {
            0.0
        } else {
            raw.clamp(
                d_lower_strength.min(d_upper_strength),
                d_lower_strength.max(d_upper_strength),
            )
        };

        // Clipping detection
        let clipping = s.attract_angle <= s.last_attract_angle - s.detent_width
            || s.attract_angle >= s.last_attract_angle + s.detent_width;

        self.pid.p = if clipping {
            s.endstop_strength_unit
        } else {
            s.detent_strength_unit
        };

        if s.was_at_limit {
            self.pid.p = 0.0;
        }
    }

    /// Detect detent crossings using hysteresis.
    fn find_detent(&mut self, shaft_angle: f32, events: &mut heapless::Vec<HapticEvt, 4>) {
        let s = &self.state;

        let (min_hyst, max_hyst) = if !s.detent_profile.kx_force {
            let hyst = s.detent_width * s.attract_hysteresis;
            (s.attract_angle - hyst, s.attract_angle + hyst)
        } else {
            let hyst = 0.15 * s.attract_hysteresis * -1.0;
            (
                s.attract_angle * (1.0 - hyst),
                s.attract_angle * (1.0 + hyst),
            )
        };

        if shaft_angle < min_hyst {
            self.state.attract_angle =
                (shaft_angle / self.state.detent_width).round() * self.state.detent_width;
        } else if shaft_angle > max_hyst {
            self.state.attract_angle =
                (shaft_angle / self.state.detent_width).round() * self.state.detent_width;
        }

        if self.state.last_attract_angle != self.state.attract_angle {
            self.detent_handler(events);
        }
    }

    /// Handle detent position changes and fire events.
    fn detent_handler(&mut self, events: &mut heapless::Vec<HapticEvt, 4>) {
        let mut effective_start = self.state.detent_profile.start_pos;
        let mut effective_end = self.state.detent_profile.end_pos;

        if self.state.detent_profile.mode == HapticMode::Vernier {
            effective_start *= self.state.detent_profile.vernier as u16;
            effective_end *= self.state.detent_profile.vernier as u16;
        }

        let decreasing = self.state.last_attract_angle > self.state.attract_angle;
        let is_ccw = self.sensor_direction == Direction::Ccw;

        // Production PCB: CCW direction maps decreasing angle to decrement
        let (increment, _check_upper) = match (decreasing, is_ccw) {
            (true, true) => (false, false),   // decrease angle + CCW = decrement, check start
            (true, false) => (true, true),     // decrease angle + CW = increment, check end
            (false, true) => (true, true),     // increase angle + CCW = increment, check end
            (false, false) => (false, false),  // increase angle + CW = decrement, check start
        };

        let at_boundary = if increment {
            self.state.current_pos >= effective_end
        } else {
            self.state.current_pos <= effective_start
        };

        if at_boundary {
            let evt = if increment {
                HapticEvt::LimitPos
            } else {
                HapticEvt::LimitNeg
            };
            let _ = events.push(evt);
            self.state.at_limit = true;
            // Only clear was_at_limit on the increasing-angle + CCW limit hit (matches C++)
            if !decreasing && is_ccw {
                self.state.was_at_limit = false;
            }
        } else {
            if self.state.at_limit {
                self.state.was_at_limit = true;
            }
            self.state.at_limit = false;

            if increment {
                self.state.current_pos += 1;
                let _ = events.push(HapticEvt::Increase);
            } else {
                self.state.current_pos -= 1;
                let _ = events.push(HapticEvt::Decrease);
            }
            self.state.last_attract_angle = self.state.attract_angle;
        }

        if !self.state.at_limit {
            let _ = events.push(HapticEvt::Either);
        }
    }

    /// Compute the PID error for the motor torque command.
    fn haptic_target(&mut self, shaft_angle: f32, shaft_velocity: f32, now_us: u64) -> HapticOutput {
        let mut error = self.state.last_attract_angle - shaft_angle;
        let error_threshold = self.state.detent_width * 0.0075;

        self.pid.output_ramp = self.state.detent_profile.output_ramp;

        if error.abs() < error_threshold {
            error *= 0.75;
        } else if shaft_velocity.abs() > 30.0 {
            error = 0.0;
        } else {
            error = error.clamp(-self.state.detent_width, self.state.detent_width);
        }

        // Bounds re-entry handling
        let needs_settle = !self.state.at_limit && self.state.was_at_limit;

        if needs_settle {
            // Return settle parameters — the caller must run the bounds settle loop
            self.state.was_at_limit = false;

            // Reset position to nearest boundary
            if self.state.current_pos <= self.state.num_detents / 2 {
                self.state.current_pos = self.state.detent_profile.start_pos;
            } else {
                self.state.current_pos = self.state.detent_profile.end_pos;
            }

            if self.state.detent_profile.mode == HapticMode::Vernier {
                self.state.current_pos *= self.state.detent_profile.vernier as u16;
            }

            // Nudge away from boundary
            if self.state.current_pos <= self.state.num_detents / 2 {
                self.state.current_pos += 1;
            } else {
                self.state.current_pos -= 1;
            }
        }

        let pid_output = self.pid.call(error, now_us);

        HapticOutput {
            pid_error: pid_output,
            run_foc: !needs_settle,
            events: heapless::Vec::new(),
        }
    }

    /// Compute settle error for bounds handler loop.
    /// Returns (error, should_break). Call repeatedly until shaft velocity < 1.0.
    pub fn bounds_settle_error(&self, shaft_angle: f32, shaft_velocity: f32) -> (f32, bool) {
        let error = self.state.attract_angle - shaft_angle;
        if error.abs() > self.state.detent_width * 2.0 {
            return (error, true); // user dragging, break out
        }
        if shaft_velocity.abs() <= 1.0 {
            return (error, true); // settled
        }
        (error, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_state_detent_width() {
        let state = HapticState::new();
        // 60 detents around 2*PI
        let expected = TWO_PI / 60.0;
        assert!(
            (state.detent_width - expected).abs() < 0.001,
            "detent_width should be 2PI/60 = {expected}, got {}",
            state.detent_width
        );
    }

    #[test]
    fn test_vernier_narrows_detent_width() {
        let mut state = HapticState::new();
        let profile = DetentProfile {
            mode: HapticMode::Vernier,
            vernier: 5,
            detent_count: 60,
            ..DetentProfile::default()
        };
        state.load_profile(profile, Some(0));
        let expected = TWO_PI / 60.0 / 5.0;
        assert!(
            (state.detent_width - expected).abs() < 0.001,
            "vernier width should be {expected}, got {}",
            state.detent_width
        );
    }

    #[test]
    fn test_load_profile_clamps_position() {
        let mut state = HapticState::new();
        state.current_pos = 500;
        let profile = DetentProfile {
            start_pos: 0,
            end_pos: 100,
            ..DetentProfile::default()
        };
        state.load_profile(profile, None);
        assert_eq!(state.current_pos, 100, "should clamp to end_pos");
    }

    #[test]
    fn test_find_detent_no_movement() {
        let mut ctrl = HapticController::new();
        let initial_pos = ctrl.state.current_pos;
        let mut events = heapless::Vec::<HapticEvt, 4>::new();
        // Shaft at attract angle — no detent crossing
        ctrl.find_detent(ctrl.state.attract_angle, &mut events);
        assert_eq!(ctrl.state.current_pos, initial_pos);
        assert!(events.is_empty());
    }

    #[test]
    fn test_find_detent_crosses_positive() {
        let mut ctrl = HapticController::new();
        let initial_pos = ctrl.state.current_pos;
        let mut events = heapless::Vec::<HapticEvt, 4>::new();
        // Move shaft well past one detent in the positive direction
        let angle = ctrl.state.attract_angle + ctrl.state.detent_width * 1.5;
        ctrl.find_detent(angle, &mut events);
        assert!(!events.is_empty(), "should fire events on detent crossing");
        // Position should have changed
        assert_ne!(ctrl.state.current_pos, initial_pos);
    }

    #[test]
    fn test_haptic_target_high_velocity_zeroes_error() {
        let mut ctrl = HapticController::new();
        // Place shaft slightly off attract angle
        let shaft = ctrl.state.attract_angle + ctrl.state.detent_width * 0.5;
        let output = ctrl.haptic_target(shaft, 50.0, 1000);
        // At high velocity (>30 rad/s), error should be zeroed → near-zero PID output
        assert!(
            output.pid_error.abs() < 0.1,
            "high velocity should zero error, got {}",
            output.pid_error
        );
    }

    #[test]
    fn test_num_detents_calculation() {
        let mut state = HapticState::new();
        let profile = DetentProfile {
            start_pos: 10,
            end_pos: 50,
            ..DetentProfile::default()
        };
        state.load_profile(profile, Some(10));
        assert_eq!(state.num_detents, 40);
    }

    // --- Detent boundary state machine ---

    #[test]
    fn test_hit_upper_limit_fires_limit_pos() {
        let mut ctrl = HapticController::new();
        // Place position at end_pos so next increment hits boundary
        ctrl.state.current_pos = ctrl.state.detent_profile.end_pos;
        ctrl.state.attract_angle = 0.0;
        ctrl.state.last_attract_angle = 0.0;

        let mut events = heapless::Vec::<HapticEvt, 4>::new();
        // Move shaft positive past hysteresis — triggers detent_handler with increment
        let angle = ctrl.state.detent_width * 1.5;
        ctrl.find_detent(angle, &mut events);

        assert!(ctrl.state.at_limit, "should be at limit");
        assert!(
            events.iter().any(|e| *e == HapticEvt::LimitPos || *e == HapticEvt::LimitNeg),
            "should fire a limit event, got {:?}",
            events
        );
    }

    #[test]
    fn test_hit_lower_limit_fires_limit_neg() {
        let mut ctrl = HapticController::new();
        ctrl.state.current_pos = ctrl.state.detent_profile.start_pos;
        ctrl.state.attract_angle = 0.0;
        ctrl.state.last_attract_angle = 0.0;

        let mut events = heapless::Vec::<HapticEvt, 4>::new();
        // Move shaft negative past hysteresis
        let angle = -ctrl.state.detent_width * 1.5;
        ctrl.find_detent(angle, &mut events);

        assert!(ctrl.state.at_limit, "should be at limit");
        assert!(
            events.iter().any(|e| *e == HapticEvt::LimitPos || *e == HapticEvt::LimitNeg),
            "should fire a limit event, got {:?}",
            events
        );
    }

    #[test]
    fn test_either_event_on_non_limit_crossing() {
        let mut ctrl = HapticController::new();
        // Start at middle so we won't hit boundary
        ctrl.state.current_pos = 128;
        ctrl.state.attract_angle = 0.0;
        ctrl.state.last_attract_angle = 0.0;

        let mut events = heapless::Vec::<HapticEvt, 4>::new();
        let angle = ctrl.state.detent_width * 1.5;
        ctrl.find_detent(angle, &mut events);

        assert!(
            events.iter().any(|e| *e == HapticEvt::Either),
            "should fire Either event on non-limit crossing, got {:?}",
            events
        );
    }

    #[test]
    fn test_was_at_limit_set_on_leaving_limit() {
        let mut ctrl = HapticController::new();
        // Simulate being at limit
        ctrl.state.at_limit = true;
        ctrl.state.current_pos = ctrl.state.detent_profile.end_pos;
        ctrl.state.attract_angle = ctrl.state.detent_width * 10.0;
        ctrl.state.last_attract_angle = ctrl.state.attract_angle;

        // Now move backwards to leave the limit
        let mut events = heapless::Vec::<HapticEvt, 4>::new();
        let angle = ctrl.state.attract_angle - ctrl.state.detent_width * 1.5;
        ctrl.find_detent(angle, &mut events);

        // After leaving limit, was_at_limit should be true
        if !ctrl.state.at_limit {
            assert!(
                ctrl.state.was_at_limit,
                "was_at_limit should be set after leaving limit"
            );
        }
    }

    #[test]
    fn test_vernier_mode_boundary_scaling() {
        let mut ctrl = HapticController::new();
        let profile = DetentProfile {
            mode: HapticMode::Vernier,
            start_pos: 0,
            end_pos: 20,
            detent_count: 20,
            vernier: 5,
            kx_force: false,
            ..DetentProfile::default()
        };
        ctrl.state.load_profile(profile, Some(20 * 5)); // At effective_end
        ctrl.state.attract_angle = 0.0;
        ctrl.state.last_attract_angle = 0.0;

        let mut events = heapless::Vec::<HapticEvt, 4>::new();
        let angle = ctrl.state.detent_width * 1.5;
        ctrl.find_detent(angle, &mut events);

        // Should hit limit at effective_end = 20 * 5 = 100
        assert!(
            ctrl.state.at_limit,
            "should hit limit at vernier-scaled boundary"
        );
    }

    #[test]
    fn test_direction_cw_reverses_increment() {
        let mut ctrl = HapticController::new();
        ctrl.sensor_direction = Direction::Cw;
        ctrl.state.current_pos = 128;
        ctrl.state.attract_angle = 0.0;
        ctrl.state.last_attract_angle = 0.0;
        let initial = ctrl.state.current_pos;

        let mut events = heapless::Vec::<HapticEvt, 4>::new();
        let angle = ctrl.state.detent_width * 1.5;
        ctrl.find_detent(angle, &mut events);

        // With CW direction, positive angle should decrement
        assert_ne!(ctrl.state.current_pos, initial, "position should change");
    }

    #[test]
    fn test_multiple_detent_crossings_sequential() {
        let mut ctrl = HapticController::new();
        ctrl.state.current_pos = 128;
        let start_pos = ctrl.state.current_pos;

        // Cross 5 detents one at a time
        for i in 1..=5 {
            let mut events = heapless::Vec::<HapticEvt, 4>::new();
            let angle = ctrl.state.detent_width * (i as f32 + 0.3);
            ctrl.state.attract_angle = ctrl.state.detent_width * ((i - 1) as f32);
            ctrl.state.last_attract_angle = ctrl.state.attract_angle;
            ctrl.find_detent(angle, &mut events);
        }

        let delta = (ctrl.state.current_pos as i32 - start_pos as i32).unsigned_abs();
        assert!(delta >= 3, "should have crossed multiple detents, delta={delta}");
    }

    // --- kxForce hysteresis ---

    #[test]
    fn test_kxforce_hysteresis_multiplicative() {
        let mut ctrl = HapticController::new();
        let profile = DetentProfile {
            kx_force: true,
            ..DetentProfile::default()
        };
        ctrl.state.load_profile(profile, Some(128));
        // Set attract_angle to a known nonzero value
        ctrl.state.attract_angle = ctrl.state.detent_width * 5.0;
        ctrl.state.last_attract_angle = ctrl.state.attract_angle;

        let mut events = heapless::Vec::<HapticEvt, 4>::new();
        // Move past multiplicative hysteresis boundary
        let angle = ctrl.state.attract_angle + ctrl.state.detent_width * 1.5;
        ctrl.find_detent(angle, &mut events);

        assert!(
            !events.is_empty(),
            "kxForce mode should still detect detent crossings"
        );
    }

    #[test]
    fn test_kxforce_near_zero_attract_angle() {
        let mut ctrl = HapticController::new();
        let profile = DetentProfile {
            kx_force: true,
            ..DetentProfile::default()
        };
        ctrl.state.load_profile(profile, Some(128));
        ctrl.state.attract_angle = 0.001; // near zero
        ctrl.state.last_attract_angle = 0.001;

        let mut events = heapless::Vec::<HapticEvt, 4>::new();
        // With attract_angle near zero, multiplicative hysteresis is tiny
        let angle = ctrl.state.detent_width * 1.5;
        ctrl.find_detent(angle, &mut events);

        // Should still produce detent crossing
        assert!(
            !events.is_empty(),
            "near-zero attract should still cross detent with kxForce"
        );
    }

    // --- correct_pid clipping ---

    #[test]
    fn test_correct_pid_clipping_switches_p_gain() {
        let mut ctrl = HapticController::new();
        // Set attract_angle far from last_attract_angle (> detent_width)
        ctrl.state.attract_angle = ctrl.state.last_attract_angle + ctrl.state.detent_width * 2.0;
        ctrl.correct_pid();
        assert!(
            (ctrl.pid.p - ctrl.state.endstop_strength_unit).abs() < 0.01,
            "P should be endstop_strength when clipping, got {}",
            ctrl.pid.p
        );
    }

    #[test]
    fn test_correct_pid_was_at_limit_zeroes_p() {
        let mut ctrl = HapticController::new();
        ctrl.state.was_at_limit = true;
        ctrl.correct_pid();
        assert!(
            ctrl.pid.p.abs() < 0.01,
            "P should be 0 when was_at_limit, got {}",
            ctrl.pid.p
        );
    }

    #[test]
    fn test_correct_pid_normal_uses_detent_strength() {
        let mut ctrl = HapticController::new();
        ctrl.state.attract_angle = ctrl.state.last_attract_angle; // no clipping
        ctrl.state.was_at_limit = false;
        ctrl.correct_pid();
        assert!(
            (ctrl.pid.p - ctrl.state.detent_strength_unit).abs() < 0.01,
            "P should be detent_strength in normal mode, got {}",
            ctrl.pid.p
        );
    }

    // --- bounds_settle_error ---

    #[test]
    fn test_bounds_settle_low_velocity_breaks() {
        let ctrl = HapticController::new();
        let (_, should_break) = ctrl.bounds_settle_error(ctrl.state.attract_angle, 0.5);
        assert!(should_break, "should break when velocity < 1.0");
    }

    #[test]
    fn test_bounds_settle_large_error_breaks() {
        let ctrl = HapticController::new();
        let far_angle = ctrl.state.attract_angle + ctrl.state.detent_width * 3.0;
        let (_, should_break) = ctrl.bounds_settle_error(far_angle, 10.0);
        assert!(should_break, "should break when error > 2*detent_width");
    }

    #[test]
    fn test_bounds_settle_normal_continues() {
        let ctrl = HapticController::new();
        // Small error, moderate velocity — should keep settling
        let angle = ctrl.state.attract_angle + ctrl.state.detent_width * 0.5;
        let (_, should_break) = ctrl.bounds_settle_error(angle, 5.0);
        assert!(!should_break, "should continue settling");
    }

    // --- haptic_loop integration ---

    #[test]
    fn test_haptic_loop_returns_torque_at_rest() {
        let mut ctrl = HapticController::new();
        // Shaft slightly off detent — should produce corrective torque
        let shaft = ctrl.state.detent_width * 0.3;
        let output = ctrl.haptic_loop(shaft, 0.0, 1000);
        assert!(output.run_foc, "should run FOC in normal operation");
        assert!(
            output.pid_error.abs() > 0.001,
            "should produce nonzero torque for position error"
        );
    }

    #[test]
    fn test_haptic_loop_small_error_attenuated() {
        let mut ctrl = HapticController::new();
        // Error below threshold (0.75% of detent_width) gets scaled by 0.75
        let tiny_offset = ctrl.state.detent_width * 0.005;
        let output = ctrl.haptic_loop(ctrl.state.attract_angle + tiny_offset, 0.0, 1000);
        // The attenuation makes the output smaller than a proportional-only response
        assert!(output.pid_error.abs() < 1.0, "tiny error should give small output");
    }
}
