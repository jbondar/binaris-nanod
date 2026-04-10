/// PID controller with output ramp limiting.
/// Ported from SimpleFOC's PIDController.
#[derive(Debug, Clone)]
pub struct PidController {
    pub p: f32,
    pub i: f32,
    pub d: f32,
    pub limit: f32,
    pub output_ramp: f32,

    integral: f32,
    prev_error: f32,
    prev_output: f32,
    prev_ts_us: u64,
}

impl PidController {
    pub fn new(p: f32, i: f32, d: f32, limit: f32, output_ramp: f32) -> Self {
        Self {
            p,
            i,
            d,
            limit,
            output_ramp,
            integral: 0.0,
            prev_error: 0.0,
            prev_output: 0.0,
            prev_ts_us: 0,
        }
    }

    /// Compute PID output given error and current timestamp in microseconds.
    pub fn call(&mut self, error: f32, now_us: u64) -> f32 {
        let dt = if self.prev_ts_us == 0 {
            1e-3 // 1ms default on first call
        } else {
            (now_us - self.prev_ts_us) as f32 * 1e-6
        };
        self.prev_ts_us = now_us;

        // Proportional
        let p_term = self.p * error;

        // Integral
        self.integral += self.i * dt * 0.5 * (error + self.prev_error);
        self.integral = clamp(self.integral, -self.limit, self.limit);

        // Derivative
        let d_term = if dt > 0.0 {
            self.d * (error - self.prev_error) / dt
        } else {
            0.0
        };

        let mut output = p_term + self.integral + d_term;
        output = clamp(output, -self.limit, self.limit);

        // Output ramp limiting
        if self.output_ramp > 0.0 {
            let max_change = self.output_ramp * dt;
            let delta = output - self.prev_output;
            if delta.abs() > max_change {
                output = self.prev_output + max_change * delta.signum();
            }
        }

        self.prev_error = error;
        self.prev_output = output;
        output
    }

    /// Reset internal state.
    pub fn reset(&mut self) {
        self.integral = 0.0;
        self.prev_error = 0.0;
        self.prev_output = 0.0;
        self.prev_ts_us = 0;
    }
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
    fn test_proportional_only() {
        let mut pid = PidController::new(2.0, 0.0, 0.0, 100.0, 0.0);
        let out = pid.call(5.0, 1000);
        assert!((out - 10.0).abs() < 0.01, "P=2 * error=5 should be 10, got {out}");
    }

    #[test]
    fn test_output_clamped_to_limit() {
        let mut pid = PidController::new(100.0, 0.0, 0.0, 10.0, 0.0);
        let out = pid.call(5.0, 1000);
        assert!((out - 10.0).abs() < 0.01, "should be clamped to limit=10, got {out}");
    }

    #[test]
    fn test_output_ramp_limits_change() {
        let mut pid = PidController::new(100.0, 0.0, 0.0, 10000.0, 1000.0);
        // First call establishes baseline
        let out1 = pid.call(1.0, 0);
        // Second call 1ms later — ramp should limit change
        let out2 = pid.call(100.0, 1000);
        let delta = (out2 - out1).abs();
        // ramp=1000/s, dt=0.001s, max_change=1.0
        assert!(delta <= 1.01, "ramp should limit delta to ~1.0, got {delta}");
    }

    #[test]
    fn test_zero_error_gives_zero_output() {
        let mut pid = PidController::new(5.0, 0.0, 0.004, 10000.0, 0.4);
        let out = pid.call(0.0, 1000);
        assert!((out).abs() < 0.01, "zero error should give ~zero output, got {out}");
    }

    #[test]
    fn test_derivative_responds_to_change() {
        let mut pid = PidController::new(0.0, 0.0, 1.0, 10000.0, 0.0);
        pid.call(0.0, 0);
        let out = pid.call(10.0, 1000); // 10 error change in 1ms
        // D=1.0 * (10-0)/0.001 = 10000
        assert!(out > 1000.0, "D term should produce large output, got {out}");
    }

    #[test]
    fn test_negative_error() {
        let mut pid = PidController::new(2.0, 0.0, 0.0, 100.0, 0.0);
        let out = pid.call(-5.0, 1000);
        assert!((out + 10.0).abs() < 0.01, "P=2 * error=-5 should be -10, got {out}");
    }

    #[test]
    fn test_integral_accumulates_over_time() {
        // P=0, I=10, D=0 — output should grow with sustained error
        let mut pid = PidController::new(0.0, 10.0, 0.0, 1000.0, 0.0);
        let mut out = 0.0;
        for i in 0..100u64 {
            out = pid.call(1.0, i * 1000); // 1ms steps, constant error=1
        }
        // Integral should accumulate: ~10 * 0.001 * 100 * 1.0 = ~1.0
        assert!(out > 0.5, "integral should accumulate, got {out}");
    }

    #[test]
    fn test_integral_clamped_to_limit() {
        let mut pid = PidController::new(0.0, 1000.0, 0.0, 5.0, 0.0);
        for i in 0..1000u64 {
            pid.call(100.0, i * 1000);
        }
        let out = pid.call(100.0, 1_000_000);
        assert!(
            out.abs() <= 5.01,
            "integral should be clamped to limit=5, got {out}"
        );
    }

    #[test]
    fn test_integral_recovers_on_error_reversal() {
        let mut pid = PidController::new(0.0, 10.0, 0.0, 100.0, 0.0);
        // Wind up positive
        for i in 0..50u64 {
            pid.call(1.0, i * 1000);
        }
        let pos_out = pid.call(1.0, 50_000);
        // Reverse error
        for i in 51..150u64 {
            pid.call(-1.0, i * 1000);
        }
        let neg_out = pid.call(-1.0, 150_000);
        assert!(
            neg_out < pos_out,
            "integral should recover after error reversal: pos={pos_out}, neg={neg_out}"
        );
    }

    #[test]
    fn test_reset_clears_state() {
        let mut pid = PidController::new(5.0, 1.0, 0.5, 100.0, 0.0);
        for i in 0..50u64 {
            pid.call(10.0, i * 1000);
        }
        pid.reset();
        let out = pid.call(0.0, 100_000);
        assert!(
            out.abs() < 0.01,
            "after reset, zero error should give zero output, got {out}"
        );
    }
}
