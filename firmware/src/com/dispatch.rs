use nanod_math::haptic::profile::DetentProfile;
use nanod_math::profile::manager::ProfileManager;
use nanod_math::protocol::command::*;
use nanod_math::protocol::parse::{self, ParseError};
use nanod_math::protocol::serialize;

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
    /// Trigger motor recalibration.
    Recalibrate,
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
                // Extract haptic config before storing
                let haptic = payload.haptic.as_ref().map(|h| h.to_detent_profile());

                let name = payload.name.clone();
                match self.profiles.set_profile(payload) {
                    Ok(_) => {
                        let _ = self.profiles.set_active(&name);
                        let evt = serialize::message_event("info", &format!("profile '{name}' set"));
                        if let Ok(json) = serialize::serialize_event(&evt) {
                            actions.push(Action::Respond(json));
                        }
                        if let Some(profile) = haptic {
                            actions.push(Action::UpdateHaptic(profile));
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
                        // Also update haptic if it has config
                        if let Some(h) = &p.haptic {
                            actions.push(Action::UpdateHaptic(h.to_detent_profile()));
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
        }
    }
}
