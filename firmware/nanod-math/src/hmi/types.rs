use serde::{Deserialize, Serialize};

/// Maximum number of actions per button event (pressed/released).
pub const MAX_KEY_ACTIONS: usize = 5;
/// Maximum number of knob value mappings.
pub const MAX_KNOB_VALUES: usize = 8;
/// Maximum HID key codes in one keyboard report.
pub const MAX_KEY_CODES: usize = 6;

/// What a button press/release triggers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum KeyAction {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "midi")]
    Midi {
        channel: u8,
        cc: u8,
        #[serde(default)]
        val: u8,
    },
    #[serde(rename = "key")]
    Keyboard {
        #[serde(default)]
        key_codes: Vec<u8>,
    },
    #[serde(rename = "mouse")]
    Mouse { buttons: u8 },
    #[serde(rename = "gamepad")]
    Gamepad { buttons: u8 },
    #[serde(rename = "profile_change")]
    ProfileChange { profile: String },
    #[serde(rename = "profile_next")]
    ProfileNext,
    #[serde(rename = "profile_prev")]
    ProfilePrev,
}

/// Actions mapped to a single button.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeyMapping {
    #[serde(default)]
    pub pressed: Vec<KeyAction>,
    #[serde(default)]
    pub released: Vec<KeyAction>,
}

impl Default for KeyMapping {
    fn default() -> Self {
        Self {
            pressed: Vec::new(),
            released: Vec::new(),
        }
    }
}

/// What type of output a knob value produces.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KnobValueType {
    Midi,
    Mouse,
    Gamepad,
    Actions,
    Profiles,
}

/// Output configuration for a knob value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "output_type")]
pub enum KnobOutput {
    #[serde(rename = "midi")]
    Midi { channel: u8, cc: u8 },
    #[serde(rename = "mouse")]
    Mouse { axis: u8 },
    #[serde(rename = "gamepad")]
    Gamepad { axis: u8 },
}

/// A knob value mapping: maps a knob angle range to an output value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KnobValue {
    /// Bitmask of keys that must be held for this mapping to be active.
    #[serde(default)]
    pub key_state: u8,
    /// Output value range.
    #[serde(default)]
    pub value_min: f32,
    #[serde(default = "default_value_max")]
    pub value_max: f32,
    /// Quantization step (0 = continuous).
    #[serde(default)]
    pub step: f32,
    /// Wrap around at limits.
    #[serde(default)]
    pub wrap: bool,
    /// Output configuration.
    pub output: KnobOutput,
}

fn default_value_max() -> f32 {
    127.0
}

/// All knob value mappings for a profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KnobMapping {
    #[serde(default)]
    pub values: Vec<KnobValue>,
}

impl Default for KnobMapping {
    fn default() -> Self {
        Self {
            values: Vec::new(),
        }
    }
}

/// Full HMI configuration for a profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HmiConfig {
    /// Per-button key mappings (4 buttons).
    #[serde(default = "default_key_mappings")]
    pub keys: Vec<KeyMapping>,
    /// Knob value mappings.
    #[serde(default)]
    pub knob: KnobMapping,
}

fn default_key_mappings() -> Vec<KeyMapping> {
    vec![
        KeyMapping::default(),
        KeyMapping::default(),
        KeyMapping::default(),
        KeyMapping::default(),
    ]
}

impl Default for HmiConfig {
    fn default() -> Self {
        Self {
            keys: default_key_mappings(),
            knob: KnobMapping::default(),
        }
    }
}

/// Map a knob position to a value using linear interpolation and optional quantization.
///
/// `position` is the raw haptic position (0..detent_count).
/// `total_detents` is the total number of detents in the profile.
/// Returns a value in [value_min, value_max], quantized by `step` if non-zero.
pub fn map_knob_value(position: u16, total_detents: u16, config: &KnobValue) -> f32 {
    if total_detents == 0 {
        return config.value_min;
    }

    let t = position as f32 / total_detents as f32;
    let raw_value = config.value_min + t * (config.value_max - config.value_min);

    let value = if config.step > 0.0 {
        (raw_value / config.step).round() * config.step
    } else {
        raw_value
    };

    // Clamp to range
    let (lo, hi) = if config.value_min <= config.value_max {
        (config.value_min, config.value_max)
    } else {
        (config.value_max, config.value_min)
    };
    value.clamp(lo, hi)
}

/// Find the active knob value mapping for the current key state.
/// Returns the first mapping whose `key_state` matches (0 means always active).
pub fn find_active_knob_value(knob: &KnobMapping, key_state: u8) -> Option<&KnobValue> {
    // First try exact match on key_state
    for v in &knob.values {
        if v.key_state != 0 && v.key_state == key_state {
            return Some(v);
        }
    }
    // Fall back to default (key_state == 0)
    for v in &knob.values {
        if v.key_state == 0 {
            return Some(v);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn midi_knob_value(channel: u8, cc: u8) -> KnobValue {
        KnobValue {
            key_state: 0,
            value_min: 0.0,
            value_max: 127.0,
            step: 1.0,
            wrap: false,
            output: KnobOutput::Midi { channel, cc },
        }
    }

    #[test]
    fn test_map_knob_value_min() {
        let kv = midi_knob_value(1, 10);
        assert_eq!(map_knob_value(0, 60, &kv), 0.0);
    }

    #[test]
    fn test_map_knob_value_max() {
        let kv = midi_knob_value(1, 10);
        assert_eq!(map_knob_value(60, 60, &kv), 127.0);
    }

    #[test]
    fn test_map_knob_value_midpoint() {
        let kv = midi_knob_value(1, 10);
        let val = map_knob_value(30, 60, &kv);
        assert_eq!(val, 64.0); // 63.5 rounds to 64 with step=1
    }

    #[test]
    fn test_map_knob_value_no_step() {
        let kv = KnobValue {
            step: 0.0,
            ..midi_knob_value(1, 10)
        };
        let val = map_knob_value(30, 60, &kv);
        assert!((val - 63.5).abs() < 0.01);
    }

    #[test]
    fn test_map_knob_value_custom_range() {
        let kv = KnobValue {
            value_min: 10.0,
            value_max: 20.0,
            step: 0.0,
            ..midi_knob_value(1, 10)
        };
        let val = map_knob_value(50, 100, &kv);
        assert!((val - 15.0).abs() < 0.01);
    }

    #[test]
    fn test_map_knob_value_zero_detents() {
        let kv = midi_knob_value(1, 10);
        assert_eq!(map_knob_value(0, 0, &kv), 0.0);
    }

    #[test]
    fn test_map_knob_value_step_quantization() {
        let kv = KnobValue {
            step: 10.0,
            value_min: 0.0,
            value_max: 100.0,
            ..midi_knob_value(1, 10)
        };
        // 33/100 * 100 = 33.0 → round(33/10)*10 = 30
        assert_eq!(map_knob_value(33, 100, &kv), 30.0);
        // 37/100 * 100 = 37.0 → round(37/10)*10 = 40
        assert_eq!(map_knob_value(37, 100, &kv), 40.0);
    }

    #[test]
    fn test_find_active_knob_default() {
        let knob = KnobMapping {
            values: vec![midi_knob_value(1, 10)],
        };
        let active = find_active_knob_value(&knob, 0);
        assert!(active.is_some());
    }

    #[test]
    fn test_find_active_knob_key_state_match() {
        let knob = KnobMapping {
            values: vec![
                midi_knob_value(1, 10), // default (key_state=0)
                KnobValue {
                    key_state: 0x1,
                    ..midi_knob_value(2, 20)
                },
            ],
        };
        // Button A held → should select channel 2
        let active = find_active_knob_value(&knob, 0x1).unwrap();
        if let KnobOutput::Midi { channel, cc } = &active.output {
            assert_eq!(*channel, 2);
            assert_eq!(*cc, 20);
        } else {
            panic!("expected MIDI output");
        }
    }

    #[test]
    fn test_find_active_knob_falls_back_to_default() {
        let knob = KnobMapping {
            values: vec![
                midi_knob_value(1, 10),
                KnobValue {
                    key_state: 0x1,
                    ..midi_knob_value(2, 20)
                },
            ],
        };
        // Button B held (0x2) — no match, falls back to default
        let active = find_active_knob_value(&knob, 0x2).unwrap();
        if let KnobOutput::Midi { channel, .. } = &active.output {
            assert_eq!(*channel, 1);
        } else {
            panic!("expected MIDI output");
        }
    }

    #[test]
    fn test_find_active_knob_empty() {
        let knob = KnobMapping { values: vec![] };
        assert!(find_active_knob_value(&knob, 0).is_none());
    }

    #[test]
    fn test_key_action_serde_roundtrip() {
        let action = KeyAction::Midi {
            channel: 1,
            cc: 64,
            val: 127,
        };
        let json = serde_json::to_string(&action).unwrap();
        let parsed: KeyAction = serde_json::from_str(&json).unwrap();
        assert_eq!(action, parsed);
    }

    #[test]
    fn test_hmi_config_serde_defaults() {
        let config: HmiConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(config.keys.len(), 4);
        assert!(config.knob.values.is_empty());
    }

    #[test]
    fn test_knob_value_serde_roundtrip() {
        let kv = midi_knob_value(1, 74);
        let json = serde_json::to_string(&kv).unwrap();
        let parsed: KnobValue = serde_json::from_str(&json).unwrap();
        assert_eq!(kv, parsed);
    }
}
