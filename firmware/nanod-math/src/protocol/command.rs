use serde::{Deserialize, Serialize};

use crate::haptic::profile::{DetentProfile, HapticMode};

/// Inbound command from host → device.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// Upload/update the current profile's haptic config.
    SetProfile(ProfilePayload),
    /// Update device settings.
    SetSettings(SettingsPayload),
    /// Motor control commands (recalibrate, etc).
    Motor(MotorCommand),
    /// Screen layout command (forwarded to LCD thread).
    Screen(ScreenCommand),
    /// Persist current state to SPIFFS.
    Save,
    /// Load a profile by name from SPIFFS.
    Load(String),
    /// List all stored profile names.
    List,
    /// Get current profile as JSON.
    Get,
    /// Get current device settings as JSON.
    GetSettings,
}

/// Haptic profile payload sent from host.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HapticConfig {
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub start_pos: u16,
    #[serde(default)]
    pub end_pos: u16,
    #[serde(default)]
    pub detent_count: u16,
    #[serde(default)]
    pub vernier: u8,
    #[serde(default)]
    pub kx_force: bool,
    #[serde(default)]
    pub output_ramp: f32,
    #[serde(default)]
    pub detent_strength: f32,
}

impl HapticConfig {
    pub fn to_detent_profile(&self) -> DetentProfile {
        let mode = match self.mode.as_str() {
            "vernier" | "VERNIER" => HapticMode::Vernier,
            "viscose" | "VISCOSE" => HapticMode::Viscose,
            "spring" | "SPRING" => HapticMode::Spring,
            _ => HapticMode::Regular,
        };
        DetentProfile {
            mode,
            start_pos: self.start_pos,
            end_pos: self.end_pos,
            detent_count: self.detent_count,
            vernier: self.vernier,
            kx_force: self.kx_force,
            output_ramp: self.output_ramp,
            detent_strength: self.detent_strength,
        }
    }

    pub fn from_detent_profile(p: &DetentProfile) -> Self {
        let mode = match p.mode {
            HapticMode::Regular => "regular",
            HapticMode::Vernier => "vernier",
            HapticMode::Viscose => "viscose",
            HapticMode::Spring => "spring",
        };
        Self {
            mode: mode.to_string(),
            start_pos: p.start_pos,
            end_pos: p.end_pos,
            detent_count: p.detent_count,
            vernier: p.vernier,
            kx_force: p.kx_force,
            output_ramp: p.output_ramp,
            detent_strength: p.detent_strength,
        }
    }
}

/// Full profile payload (name + haptic config; HMI/LED/audio added in later phases).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfilePayload {
    pub name: String,
    #[serde(default)]
    pub haptic: Option<HapticConfig>,
    // Future phases:
    // pub hmi: Option<HmiConfig>,
    // pub led: Option<LedConfig>,
    // pub audio: Option<AudioConfig>,
}

/// Device settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SettingsPayload {
    #[serde(default)]
    pub midi_channel: Option<u8>,
    #[serde(default)]
    pub orientation: Option<u8>,
    #[serde(default)]
    pub led_brightness: Option<u8>,
    #[serde(default)]
    pub idle_timeout_s: Option<u16>,
}

impl Default for SettingsPayload {
    fn default() -> Self {
        Self {
            midi_channel: Some(1),
            orientation: Some(0),
            led_brightness: Some(200),
            idle_timeout_s: Some(30),
        }
    }
}

/// Motor control commands.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MotorCommand {
    #[serde(default)]
    pub recalibrate: bool,
}

/// Screen layout command (placeholder for Phase 4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScreenCommand {
    #[serde(default)]
    pub layout: String,
    #[serde(default)]
    pub data: serde_json::Value,
}

/// Outbound event from device → host.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Event {
    Angle(AngleEvent),
    Key(KeyEvent),
    Message(MessageEvent),
    ProfileResponse(ProfileResponse),
    SettingsResponse(SettingsResponse),
    ListResponse(ListResponse),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AngleEvent {
    pub angle: AngleData,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AngleData {
    pub cur_pos: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeyEvent {
    pub key: KeyData,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeyData {
    pub num: u8,
    pub state: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageEvent {
    pub msg: MessageData,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageData {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileResponse {
    pub profile: ProfilePayload,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SettingsResponse {
    pub settings: SettingsPayload,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListResponse {
    pub profiles: Vec<String>,
}
