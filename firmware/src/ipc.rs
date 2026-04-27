//! Inter-thread communication channels.
//!
//! Each thread receives a context struct with its channel endpoints.
//! Channels are created in main() and distributed to thread spawn functions.
//! Endpoints are passed through `xTaskCreatePinnedToCore`'s `*mut c_void`
//! via `Box::into_raw` / `Box::from_raw`.

use std::sync::mpsc::{Receiver, SyncSender};

use nanod_math::haptic::profile::DetentProfile;
use nanod_math::led::types::LedConfig;
use nanod_math::protocol::command::KeyEvent;

/// Snapshot of motor angle data, published by FOC thread.
pub struct AngleSnapshot {
    /// Continuous shaft angle in radians (multi-turn).
    pub shaft_angle: f32,
    /// Current haptic position (0..end_pos).
    pub current_pos: u16,
}

/// Commands sent from COM thread to HMI thread.
pub enum HmiCommand {
    /// Update LED configuration from a profile.
    UpdateLedConfig(LedConfig),
    /// Update device settings relevant to HMI (brightness, orientation).
    UpdateSettings {
        brightness: u8,
        orientation: u8,
    },
}

/// Display modes.
#[derive(Debug, Clone, PartialEq)]
pub enum DisplayMode {
    Value,
    Media,
}

/// Commands sent from COM thread to display thread.
pub enum DisplayCommand {
    /// Switch display mode (value screen vs media controller).
    SetMode(DisplayMode),
    /// Update media track metadata.
    MediaMeta {
        title: String,
        artist: String,
        duration_s: u32,
        position_s: u32,
        playing: bool,
    },
    /// Album art chunk (raw RGB565 bytes, already decoded from base64).
    MediaArtChunk { offset: u32, data: Vec<u8> },
    /// Art transfer starting — hide canvas to prevent scanline artifacts.
    MediaArtStart,
    /// All art chunks sent — swap buffers and display the image.
    MediaArtDone,
}

/// Commands sent from COM thread to FOC thread.
pub enum FocCommand {
    /// Update the haptic detent profile.
    UpdateHaptic(DetentProfile),
    /// Trigger motor recalibration.
    Recalibrate,
}

/// Channel endpoints for the FOC thread.
pub struct FocContext {
    /// Receive haptic profile updates and recalibrate commands from COM.
    pub cmd_rx: Receiver<FocCommand>,
    /// Publish angle snapshots to HMI.
    pub angle_tx: SyncSender<AngleSnapshot>,
    /// Publish angle snapshots to display thread.
    pub display_tx: SyncSender<AngleSnapshot>,
}

/// Channel endpoints for the COM thread.
pub struct ComContext {
    /// Send haptic/motor commands to FOC.
    pub foc_tx: SyncSender<FocCommand>,
    /// Send LED/settings updates to HMI.
    pub hmi_tx: SyncSender<HmiCommand>,
    /// Send display commands (media mode, metadata, art).
    pub display_tx: SyncSender<DisplayCommand>,
    /// Receive key events from HMI for serial output.
    pub key_rx: Receiver<KeyEvent>,
}

/// Channel endpoints for the HMI thread.
pub struct HmiContext {
    /// Receive angle snapshots from FOC.
    pub angle_rx: Receiver<AngleSnapshot>,
    /// Receive config updates from COM.
    pub cmd_rx: Receiver<HmiCommand>,
    /// Send key events to COM for serial output.
    pub key_tx: SyncSender<KeyEvent>,
}
