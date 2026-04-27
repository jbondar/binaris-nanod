use nanod_math::haptic::profile::{DetentProfile, HapticMode};
use nanod_math::led::types::LedConfig;
use nanod_math::profile::manager::ProfileManager;
use nanod_math::protocol::command::*;
use nanod_math::protocol::parse;
use nanod_math::protocol::serialize;

use crate::ipc::{DisplayCommand, DisplayMode};

/// Handles parsed commands and produces responses.
/// Owns the profile manager and settings state.
pub struct Dispatcher {
    pub profiles: ProfileManager,
    pub settings: SettingsPayload,
}

/// Action the COM thread should take after dispatching a command.
pub enum Action {
    /// Send this JSON string over serial.
    Respond(String),
    /// Update the FOC thread's haptic profile.
    UpdateHaptic(DetentProfile),
    /// Update HMI thread's LED configuration.
    UpdateLedConfig(LedConfig),
    /// Update HMI thread's settings (brightness, orientation).
    UpdateSettings { brightness: u8, orientation: u8 },
    /// Trigger motor recalibration.
    Recalibrate,
    /// Send a command to the display thread.
    Display(DisplayCommand),
    /// Enter binary receive mode for album art (raw bytes from stdin).
    BinaryArtReceive(u32),
    /// Persist profiles/settings to SPIFFS.
    Save,
    /// No action needed.
    None,
}

impl Dispatcher {
    pub fn new() -> Self {
        Self {
            profiles: ProfileManager::new(),
            settings: SettingsPayload::default(),
        }
    }

    /// Process a raw JSON line from serial. Returns actions to take.
    pub fn handle_line(&mut self, line: &str) -> Vec<Action> {
        let mut actions = Vec::new();

        match parse::parse_command(line) {
            Ok(cmd) => self.handle_command(cmd, &mut actions),
            Err(e) => {
                let evt = serialize::message_event("error", &e.to_string());
                if let Ok(json) = serialize::serialize_event(&evt) {
                    actions.push(Action::Respond(json));
                }
            }
        }

        actions
    }

    fn handle_command(&mut self, cmd: Command, actions: &mut Vec<Action>) {
        match cmd {
            Command::SetProfile(payload) => {
                // Extract configs before storing
                let haptic = payload.haptic.as_ref().map(|h| h.to_detent_profile());
                let led = payload.led.clone();

                let name = payload.name.clone();
                match self.profiles.set_profile(payload) {
                    Ok(_) => {
                        let _ = self.profiles.set_active(&name);
                        let evt =
                            serialize::message_event("info", &format!("profile '{name}' set"));
                        if let Ok(json) = serialize::serialize_event(&evt) {
                            actions.push(Action::Respond(json));
                        }
                        if let Some(profile) = haptic {
                            actions.push(Action::UpdateHaptic(profile));
                        }
                        if let Some(led_config) = led {
                            actions.push(Action::UpdateLedConfig(led_config));
                        }
                    }
                    Err(e) => {
                        let evt = serialize::message_event("error", &e.to_string());
                        if let Ok(json) = serialize::serialize_event(&evt) {
                            actions.push(Action::Respond(json));
                        }
                    }
                }
            }

            Command::SetSettings(s) => {
                if let Some(v) = s.midi_channel {
                    self.settings.midi_channel = Some(v);
                }
                if let Some(v) = s.orientation {
                    self.settings.orientation = Some(v);
                }
                if let Some(v) = s.led_brightness {
                    self.settings.led_brightness = Some(v);
                }
                if let Some(v) = s.idle_timeout_s {
                    self.settings.idle_timeout_s = Some(v);
                }
                let evt = serialize::message_event("info", "settings updated");
                if let Ok(json) = serialize::serialize_event(&evt) {
                    actions.push(Action::Respond(json));
                }
                // Forward brightness/orientation to HMI thread
                let brightness = self.settings.led_brightness.unwrap_or(200);
                let orientation = self.settings.orientation.unwrap_or(0);
                actions.push(Action::UpdateSettings {
                    brightness,
                    orientation,
                });
            }

            Command::Motor(m) => {
                if m.recalibrate {
                    actions.push(Action::Recalibrate);
                    let evt = serialize::message_event("info", "recalibrating motor");
                    if let Ok(json) = serialize::serialize_event(&evt) {
                        actions.push(Action::Respond(json));
                    }
                }
            }

            Command::Screen(_) => {
                // Phase 4 — forward to LCD thread
                let evt = serialize::message_event("info", "screen commands not yet implemented");
                if let Ok(json) = serialize::serialize_event(&evt) {
                    actions.push(Action::Respond(json));
                }
            }

            Command::Save => {
                actions.push(Action::Save);
                let evt = serialize::message_event("info", "saving to flash");
                if let Ok(json) = serialize::serialize_event(&evt) {
                    actions.push(Action::Respond(json));
                }
            }

            Command::Load(name) => {
                if self.profiles.get_profile(&name).is_some() {
                    let _ = self.profiles.set_active(&name);
                    // Send back the loaded profile
                    if let Some(p) = self.profiles.active_profile() {
                        let evt = serialize::profile_response(p.clone());
                        if let Ok(json) = serialize::serialize_event(&evt) {
                            actions.push(Action::Respond(json));
                        }
                        if let Some(h) = &p.haptic {
                            actions.push(Action::UpdateHaptic(h.to_detent_profile()));
                        }
                        if let Some(l) = &p.led {
                            actions.push(Action::UpdateLedConfig(l.clone()));
                        }
                    }
                } else {
                    let evt =
                        serialize::message_event("error", &format!("profile '{name}' not found"));
                    if let Ok(json) = serialize::serialize_event(&evt) {
                        actions.push(Action::Respond(json));
                    }
                }
            }

            Command::List => {
                let names = self.profiles.list_names();
                let evt = serialize::list_response(names);
                if let Ok(json) = serialize::serialize_event(&evt) {
                    actions.push(Action::Respond(json));
                }
            }

            Command::Get => {
                if let Some(p) = self.profiles.active_profile() {
                    let evt = serialize::profile_response(p.clone());
                    if let Ok(json) = serialize::serialize_event(&evt) {
                        actions.push(Action::Respond(json));
                    }
                } else {
                    let evt = serialize::message_event("error", "no active profile");
                    if let Ok(json) = serialize::serialize_event(&evt) {
                        actions.push(Action::Respond(json));
                    }
                }
            }

            Command::GetSettings => {
                let evt = serialize::settings_response(self.settings.clone());
                if let Ok(json) = serialize::serialize_event(&evt) {
                    actions.push(Action::Respond(json));
                }
            }

            // --- Media controller commands ---

            Command::MediaMode(on) => {
                let mode = if on { DisplayMode::Media } else { DisplayMode::Value };
                actions.push(Action::Display(DisplayCommand::SetMode(mode)));
                let state = if on { "on" } else { "off" };
                let evt = serialize::message_event("info", &format!("media mode {state}"));
                if let Ok(json) = serialize::serialize_event(&evt) {
                    actions.push(Action::Respond(json));
                }
            }

            Command::MediaMeta(meta) => {
                actions.push(Action::Display(DisplayCommand::MediaMeta {
                    title: meta.title,
                    artist: meta.artist,
                    duration_s: meta.duration,
                    position_s: meta.position,
                    playing: meta.playing,
                }));
            }

            Command::MediaArt(art) => {
                let decoded = decode_base64(&art.data);
                actions.push(Action::Display(DisplayCommand::MediaArtChunk {
                    offset: art.offset,
                    data: decoded,
                }));
            }

            Command::MediaArtDone => {
                actions.push(Action::Display(DisplayCommand::MediaArtDone));
            }

            Command::MediaArtBin(size) => {
                // Signal to COM thread to enter binary receive mode
                actions.push(Action::BinaryArtReceive(size));
            }

            Command::MediaHaptic(mode) => {
                let profile = match mode.as_str() {
                    "volume" => DetentProfile {
                        mode: HapticMode::Regular,
                        start_pos: 0,
                        end_pos: 100,
                        detent_count: 20,
                        vernier: 0,
                        kx_force: false,
                        output_ramp: 5000.0,
                        detent_strength: 3.0,
                    },
                    "scrub" | _ => DetentProfile {
                        mode: HapticMode::Viscose,
                        start_pos: 0,
                        end_pos: 255,
                        detent_count: 0,
                        vernier: 0,
                        kx_force: false,
                        output_ramp: 5000.0,
                        detent_strength: 0.0,
                    },
                };
                actions.push(Action::UpdateHaptic(profile));
                let evt = serialize::message_event("info", &format!("media haptic: {mode}"));
                if let Ok(json) = serialize::serialize_event(&evt) {
                    actions.push(Action::Respond(json));
                }
            }
        }
    }
}

/// Decode base64-encoded data to raw bytes.
fn decode_base64(input: &str) -> Vec<u8> {
    fn val(c: u8) -> u8 {
        match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => 0,
        }
    }

    let bytes: Vec<u8> = input
        .bytes()
        .filter(|&b| b != b'=' && b != b'\n' && b != b'\r' && b != b' ')
        .collect();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);

    for chunk in bytes.chunks(4) {
        let n = chunk.len();
        if n < 2 {
            break;
        }
        let a = val(chunk[0]);
        let b = val(chunk[1]);
        let c = if n > 2 { val(chunk[2]) } else { 0 };
        let d = if n > 3 { val(chunk[3]) } else { 0 };

        out.push((a << 2) | (b >> 4));
        if n > 2 {
            out.push((b << 4) | (c >> 2));
        }
        if n > 3 {
            out.push((c << 6) | d);
        }
    }
    out
}
