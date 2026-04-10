use core::f32::consts::PI;

const TWO_PI: f32 = 2.0 * PI;
const SQRT3: f32 = 1.732_050_8;

/// Motor electrical and mechanical constants.
pub struct MotorConfig {
    pub pole_pairs: u8,
    pub phase_resistance: f32,
    pub voltage_supply: f32,
    pub voltage_limit: f32,
    pub current_limit: f32,
    pub velocity_lpf_tf: f32,
}

impl Default for MotorConfig {
    fn default() -> Self {
        Self {
            pole_pairs: 7,
            phase_resistance: 5.3,
            voltage_supply: 4.75,
            voltage_limit: 4.5,
            current_limit: 1.22,
            velocity_lpf_tf: 0.01,
        }
    }
}

/// Three-phase duty cycles (0.0 to 1.0).
#[derive(Debug, Clone, Copy, Default)]
pub struct PhaseDuty {
    pub a: f32,
    pub b: f32,
    pub c: f32,
}

/// FOC motor state — pure math, no hardware deps.
pub struct FocState {
    pub config: MotorConfig,

    // Sensor state
    pub shaft_angle: f32,
    pub electrical_angle: f32,
    pub shaft_velocity: f32,
    pub sensor_offset: f32,
    pub sensor_direction: i8, // 1 = CW, -1 = CCW

    // Velocity LPF state
    prev_angle: f32,
    prev_velocity: f32,
    prev_ts_us: u64,

    // Zero electrical angle from calibration
    pub zero_electrical_angle: f32,
}

impl FocState {
    pub fn new(config: MotorConfig) -> Self {
        Self {
            config,
            shaft_angle: 0.0,
            electrical_angle: 0.0,
            shaft_velocity: 0.0,
            sensor_offset: 0.0,
            sensor_direction: 1,
            prev_angle: 0.0,
            prev_velocity: 0.0,
            prev_ts_us: 0,
            zero_electrical_angle: 0.0,
        }
    }

    /// Update shaft angle, velocity, and electrical angle from raw sensor reading.
    pub fn update_sensor(&mut self, raw_angle: f32, now_us: u64) {
        self.shaft_angle = raw_angle;

        // Low-pass filtered velocity
        let dt = if self.prev_ts_us == 0 {
            1e-3
        } else {
            (now_us - self.prev_ts_us) as f32 * 1e-6
        };

        if dt > 0.0 && self.prev_ts_us != 0 {
            let raw_velocity = (self.shaft_angle - self.prev_angle) / dt;
            let tf = self.config.velocity_lpf_tf;
            self.shaft_velocity = (raw_velocity * dt + self.prev_velocity * tf) / (dt + tf);
        }

        self.prev_angle = self.shaft_angle;
        self.prev_velocity = self.shaft_velocity;
        self.prev_ts_us = now_us;

        // Electrical angle
        self.electrical_angle = normalize_angle(
            (self.sensor_direction as f32)
                * (self.shaft_angle * self.config.pole_pairs as f32)
                - self.zero_electrical_angle,
        );
    }

    /// Compute SVPWM duty cycles for torque-mode FOC.
    /// torque_command is the PID output → converted to Uq voltage.
    pub fn compute_torque(&self, torque_command: f32) -> PhaseDuty {
        // Torque → voltage (SimpleFOC torque mode with phase resistance)
        let uq = torque_command * self.config.phase_resistance;
        let uq = clamp(uq, -self.config.voltage_limit, self.config.voltage_limit);
        let ud = 0.0; // No d-axis voltage in simple torque mode

        set_phase_voltage(uq, ud, self.electrical_angle, self.config.voltage_limit)
    }
}

/// Inverse Park transform + Space Vector PWM.
/// Returns three-phase duty cycles (0.0 to 1.0).
pub fn set_phase_voltage(uq: f32, ud: f32, angle: f32, voltage_limit: f32) -> PhaseDuty {
    let uref = (ud * ud + uq * uq).sqrt();
    let uref = uref.min(voltage_limit);

    // Angle of the voltage vector
    let angle_el = normalize_angle(angle + (ud.atan2(uq)));

    // SVPWM sector determination (6 sectors, 60 degrees each)
    let sector = ((angle_el / (PI / 3.0)) as i32 + 1).clamp(1, 6) as u8;

    // Time segments within the sector
    let t1;
    let t2;
    let sector_angle = angle_el - (sector as f32 - 1.0) * PI / 3.0;

    // Normalize voltage to maximum modulation index
    let u_norm = SQRT3 * uref / voltage_limit;

    t1 = u_norm * (PI / 3.0 - sector_angle).sin();
    t2 = u_norm * sector_angle.sin();

    let t0 = 1.0 - t1 - t2;

    // Duty cycles per sector
    let (da, db, dc) = match sector {
        1 => (t1 + t2 + t0 / 2.0, t2 + t0 / 2.0, t0 / 2.0),
        2 => (t1 + t0 / 2.0, t1 + t2 + t0 / 2.0, t0 / 2.0),
        3 => (t0 / 2.0, t1 + t2 + t0 / 2.0, t2 + t0 / 2.0),
        4 => (t0 / 2.0, t1 + t0 / 2.0, t1 + t2 + t0 / 2.0),
        5 => (t2 + t0 / 2.0, t0 / 2.0, t1 + t2 + t0 / 2.0),
        6 => (t1 + t2 + t0 / 2.0, t0 / 2.0, t1 + t0 / 2.0),
        _ => (0.0, 0.0, 0.0),
    };

    PhaseDuty {
        a: da.clamp(0.0, 1.0),
        b: db.clamp(0.0, 1.0),
        c: dc.clamp(0.0, 1.0),
    }
}

/// Normalize angle to [0, 2*PI).
pub fn normalize_angle(mut angle: f32) -> f32 {
    angle %= TWO_PI;
    if angle < 0.0 {
        angle += TWO_PI;
    }
    angle
}

fn clamp(v: f32, min: f32, max: f32) -> f32 {
    if v < min {
        min
    } else if v > max {
        max
    } else {
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_angle_positive() {
        let a = normalize_angle(7.0);
        assert!(a >= 0.0 && a < TWO_PI, "got {a}");
    }

    #[test]
    fn test_normalize_angle_negative() {
        let a = normalize_angle(-1.0);
        assert!(a >= 0.0 && a < TWO_PI, "got {a}");
        assert!((a - (TWO_PI - 1.0)).abs() < 0.001);
    }

    #[test]
    fn test_svpwm_zero_voltage() {
        let duty = set_phase_voltage(0.0, 0.0, 0.0, 5.0);
        // All phases should be 0.5 (centered) when Uref = 0
        assert!((duty.a - 0.5).abs() < 0.01, "a={}", duty.a);
        assert!((duty.b - 0.5).abs() < 0.01, "b={}", duty.b);
        assert!((duty.c - 0.5).abs() < 0.01, "c={}", duty.c);
    }

    #[test]
    fn test_svpwm_duties_in_range() {
        for angle_deg in (0..360).step_by(15) {
            let angle = (angle_deg as f32).to_radians();
            let duty = set_phase_voltage(2.0, 0.0, angle, 5.0);
            assert!(
                duty.a >= 0.0 && duty.a <= 1.0,
                "a out of range at {angle_deg}deg: {}",
                duty.a
            );
            assert!(
                duty.b >= 0.0 && duty.b <= 1.0,
                "b out of range at {angle_deg}deg: {}",
                duty.b
            );
            assert!(
                duty.c >= 0.0 && duty.c <= 1.0,
                "c out of range at {angle_deg}deg: {}",
                duty.c
            );
        }
    }

    #[test]
    fn test_svpwm_symmetry() {
        // At 0 and PI, voltage vectors should be opposite
        let d0 = set_phase_voltage(2.0, 0.0, 0.0, 5.0);
        let d180 = set_phase_voltage(2.0, 0.0, PI, 5.0);
        // Phase A at 0 should be high, at PI should be low (roughly)
        assert!(d0.a > d180.a, "expected a@0 > a@PI");
    }

    #[test]
    fn test_velocity_lpf() {
        let mut foc = FocState::new(MotorConfig::default());
        // Simulate constant rotation: 1 rad per 1ms
        for i in 0..100u64 {
            let angle = i as f32 * 0.001 * 100.0; // 100 rad/s
            foc.update_sensor(angle, i * 1000);
        }
        // After settling, velocity should approach 100 rad/s
        assert!(
            (foc.shaft_velocity - 100.0).abs() < 15.0,
            "velocity should converge to ~100, got {}",
            foc.shaft_velocity
        );
    }

    #[test]
    fn test_electrical_angle_wraps() {
        let mut foc = FocState::new(MotorConfig::default());
        foc.sensor_direction = 1;
        foc.zero_electrical_angle = 0.0;
        foc.update_sensor(10.0 * TWO_PI, 1000);
        assert!(
            foc.electrical_angle >= 0.0 && foc.electrical_angle < TWO_PI,
            "electrical angle should be normalized, got {}",
            foc.electrical_angle
        );
    }

    #[test]
    fn test_torque_to_duty() {
        let foc = FocState::new(MotorConfig::default());
        let duty = foc.compute_torque(0.0);
        // Zero torque → near-center duty
        assert!((duty.a - 0.5).abs() < 0.1);
    }

    #[test]
    fn test_svpwm_sector_boundaries_in_range() {
        // Test at exact sector boundary angles: 0, 60, 120, 180, 240, 300
        for deg in [0, 60, 120, 180, 240, 300] {
            let angle = (deg as f32).to_radians();
            let duty = set_phase_voltage(2.0, 0.0, angle, 5.0);
            assert!(
                duty.a >= 0.0 && duty.a <= 1.0
                    && duty.b >= 0.0 && duty.b <= 1.0
                    && duty.c >= 0.0 && duty.c <= 1.0,
                "duties out of range at sector boundary {deg}deg: a={} b={} c={}",
                duty.a, duty.b, duty.c
            );
        }
    }

    #[test]
    fn test_svpwm_continuity_across_sectors() {
        // Duty cycles should not jump wildly between adjacent angles
        let step = 0.1_f32.to_radians();
        let mut prev = set_phase_voltage(2.0, 0.0, 0.0, 5.0);
        let mut angle = step;
        while angle < TWO_PI {
            let cur = set_phase_voltage(2.0, 0.0, angle, 5.0);
            let da = (cur.a - prev.a).abs();
            let db = (cur.b - prev.b).abs();
            let dc = (cur.c - prev.c).abs();
            assert!(
                da < 0.15 && db < 0.15 && dc < 0.15,
                "discontinuity at {:.1}deg: da={da:.3} db={db:.3} dc={dc:.3}",
                angle.to_degrees()
            );
            prev = cur;
            angle += step;
        }
    }

    #[test]
    fn test_svpwm_saturation_clamped() {
        // Uq > voltage_limit should be clamped
        let duty = set_phase_voltage(100.0, 0.0, 1.0, 5.0);
        assert!(
            duty.a >= 0.0 && duty.a <= 1.0
                && duty.b >= 0.0 && duty.b <= 1.0
                && duty.c >= 0.0 && duty.c <= 1.0,
            "saturated voltage should still produce valid duties"
        );
    }

    #[test]
    fn test_torque_clamped_to_voltage_limit() {
        let foc = FocState::new(MotorConfig::default());
        // Large torque command — should be clamped by voltage_limit
        let duty = foc.compute_torque(100.0);
        assert!(
            duty.a >= 0.0 && duty.a <= 1.0
                && duty.b >= 0.0 && duty.b <= 1.0
                && duty.c >= 0.0 && duty.c <= 1.0,
            "large torque should produce valid clamped duties"
        );
    }
}
