use serde::{Deserialize, Serialize};

/// 24-bit RGB color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Create from a 0xRRGGBB hex value.
    pub const fn from_hex(hex: u32) -> Self {
        Self {
            r: ((hex >> 16) & 0xFF) as u8,
            g: ((hex >> 8) & 0xFF) as u8,
            b: (hex & 0xFF) as u8,
        }
    }

    /// Apply brightness scaling (0-255). 0 = off, 255 = full.
    pub fn scaled(self, brightness: u8) -> Self {
        Self {
            r: ((self.r as u16 * brightness as u16) / 255) as u8,
            g: ((self.g as u16 * brightness as u16) / 255) as u8,
            b: ((self.b as u16 * brightness as u16) / 255) as u8,
        }
    }
}

impl Default for Rgb {
    fn default() -> Self {
        Self { r: 0, g: 0, b: 0 }
    }
}

/// Per-button idle/pressed colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ButtonColors {
    pub idle: Rgb,
    pub pressed: Rgb,
}

impl Default for ButtonColors {
    fn default() -> Self {
        Self {
            idle: Rgb::from_hex(0x08596C),
            pressed: Rgb::from_hex(0xFFFFFF),
        }
    }
}

/// LED configuration for a profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_brightness")]
    pub brightness: u8,
    #[serde(default = "default_pointer_col")]
    pub pointer_col: Rgb,
    #[serde(default = "default_primary_col")]
    pub primary_col: Rgb,
    #[serde(default = "default_secondary_col")]
    pub secondary_col: Rgb,
    #[serde(default)]
    pub button_colors: [ButtonColors; 4],
}

fn default_true() -> bool {
    true
}
fn default_brightness() -> u8 {
    100
}
fn default_pointer_col() -> Rgb {
    Rgb::from_hex(0xFFFFFF)
}
fn default_primary_col() -> Rgb {
    Rgb::from_hex(0x08596C)
}
fn default_secondary_col() -> Rgb {
    Rgb::from_hex(0x47040D)
}

impl Default for LedConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            brightness: 100,
            pointer_col: Rgb::from_hex(0xFFFFFF),
            primary_col: Rgb::from_hex(0x08596C),
            secondary_col: Rgb::from_hex(0x47040D),
            button_colors: [
                ButtonColors {
                    idle: Rgb::from_hex(0x08596C),
                    pressed: Rgb::from_hex(0xFFFFFF),
                },
                ButtonColors {
                    idle: Rgb::from_hex(0x1A2E52),
                    pressed: Rgb::from_hex(0xFFFFFF),
                },
                ButtonColors {
                    idle: Rgb::from_hex(0x200524),
                    pressed: Rgb::from_hex(0xFFFFFF),
                },
                ButtonColors {
                    idle: Rgb::from_hex(0x47040D),
                    pressed: Rgb::from_hex(0xFFFFFF),
                },
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rgb_from_hex() {
        let c = Rgb::from_hex(0x08596C);
        assert_eq!(c, Rgb::new(0x08, 0x59, 0x6C));
    }

    #[test]
    fn test_rgb_scaled_full() {
        let c = Rgb::new(100, 200, 50);
        assert_eq!(c.scaled(255), c);
    }

    #[test]
    fn test_rgb_scaled_half() {
        let c = Rgb::new(100, 200, 50);
        let s = c.scaled(128);
        // 100*128/255 ≈ 50, 200*128/255 ≈ 100, 50*128/255 ≈ 25
        assert_eq!(s.r, 50);
        assert_eq!(s.g, 100);
        assert_eq!(s.b, 25);
    }

    #[test]
    fn test_rgb_scaled_zero() {
        let c = Rgb::new(255, 255, 255);
        assert_eq!(c.scaled(0), Rgb::new(0, 0, 0));
    }

    #[test]
    fn test_led_config_default() {
        let cfg = LedConfig::default();
        assert!(cfg.enabled);
        assert_eq!(cfg.brightness, 100);
        assert_eq!(cfg.pointer_col, Rgb::from_hex(0xFFFFFF));
    }

    #[test]
    fn test_led_config_serde_roundtrip() {
        let cfg = LedConfig::default();
        let json = serde_json::to_string(&cfg).unwrap();
        let parsed: LedConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cfg, parsed);
    }

    #[test]
    fn test_led_config_serde_defaults() {
        // Empty JSON should produce defaults
        let parsed: LedConfig = serde_json::from_str("{}").unwrap();
        assert!(parsed.enabled);
        assert_eq!(parsed.brightness, 100);
    }
}
