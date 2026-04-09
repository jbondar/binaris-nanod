/// Haptic event types fired on detent state changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HapticEvt {
    Increase,
    Decrease,
    Either,
    LimitPos,
    LimitNeg,
}

/// Supported haptic texture modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HapticMode {
    /// Only coarse detents used
    Regular,
    /// Coarse with fine between (multiplied by vernier)
    Vernier,
    /// Resistance while turning
    Viscose,
    /// Snap back to center point
    Spring,
}

/// Defines the behavior of a haptic detent profile.
#[derive(Debug, Clone, Copy)]
pub struct DetentProfile {
    pub mode: HapticMode,
    pub start_pos: u16,
    pub end_pos: u16,
    pub detent_count: u16,
    pub vernier: u8,
    pub kx_force: bool,
    pub output_ramp: f32,
    pub detent_strength: f32,
}

impl Default for DetentProfile {
    fn default() -> Self {
        Self {
            mode: HapticMode::Regular,
            start_pos: 0,
            end_pos: 255,
            detent_count: 60,
            vernier: 5,
            kx_force: false,
            output_ramp: 5000.0,
            detent_strength: 3.0,
        }
    }
}

/// Angle event sent when the haptic position changes.
#[derive(Debug, Clone, Copy)]
pub struct AngleEvt {
    pub cur_pos: u16,
}

/// Motor sensor direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Unknown,
    Cw,
    Ccw,
}

/// Motor calibration data stored in NVS.
#[derive(Debug, Clone, Copy)]
pub struct MotorCalibration {
    pub direction: Direction,
    pub zero_angle: f32,
}

/// Sentinel for "not set" float values.
pub const NOT_SET: f32 = -12345.0;

impl Default for MotorCalibration {
    fn default() -> Self {
        Self {
            direction: Direction::Unknown,
            zero_angle: NOT_SET,
        }
    }
}
