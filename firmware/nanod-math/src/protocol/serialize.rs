use super::command::*;

/// Serialize an outbound event to a JSON string (newline-terminated).
pub fn serialize_event(event: &Event) -> Result<String, serde_json::Error> {
    let mut json = serde_json::to_string(event)?;
    json.push('\n');
    Ok(json)
}

/// Helper: create an angle position event.
pub fn angle_event(cur_pos: u16) -> Event {
    Event::Angle(AngleEvent {
        angle: AngleData { cur_pos },
    })
}

/// Helper: create a key event.
pub fn key_event(num: u8, state: &str) -> Event {
    Event::Key(KeyEvent {
        key: KeyData {
            num,
            state: state.to_string(),
        },
    })
}

/// Helper: create an info/error message event.
pub fn message_event(msg_type: &str, text: &str) -> Event {
    Event::Message(MessageEvent {
        msg: MessageData {
            msg_type: msg_type.to_string(),
            text: text.to_string(),
        },
    })
}

/// Helper: create a profile response.
pub fn profile_response(profile: ProfilePayload) -> Event {
    Event::ProfileResponse(ProfileResponse { profile })
}

/// Helper: create a settings response.
pub fn settings_response(settings: SettingsPayload) -> Event {
    Event::SettingsResponse(SettingsResponse { settings })
}

/// Helper: create a list response.
pub fn list_response(names: Vec<String>) -> Event {
    Event::ListResponse(ListResponse { profiles: names })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_angle_event() {
        let json = serialize_event(&angle_event(42)).unwrap();
        assert!(json.contains("\"cur_pos\":42"));
        assert!(json.ends_with('\n'));
    }

    #[test]
    fn test_serialize_key_event() {
        let json = serialize_event(&key_event(0, "pressed")).unwrap();
        assert!(json.contains("\"num\":0"));
        assert!(json.contains("\"state\":\"pressed\""));
    }

    #[test]
    fn test_serialize_message_event() {
        let json = serialize_event(&message_event("info", "calibration complete")).unwrap();
        assert!(json.contains("\"type\":\"info\""));
        assert!(json.contains("calibration complete"));
    }

    #[test]
    fn test_serialize_profile_response() {
        let profile = ProfilePayload {
            name: "test".into(),
            haptic: None,
            led: None,
        };
        let json = serialize_event(&profile_response(profile)).unwrap();
        assert!(json.contains("\"name\":\"test\""));
    }

    #[test]
    fn test_serialize_list_response() {
        let names = vec!["a".into(), "b".into(), "c".into()];
        let json = serialize_event(&list_response(names)).unwrap();
        assert!(json.contains("\"profiles\":[\"a\",\"b\",\"c\"]"));
    }

    #[test]
    fn test_serialize_roundtrip_angle() {
        let evt = angle_event(100);
        let json = serialize_event(&evt).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(json.trim()).unwrap();
        assert_eq!(parsed["angle"]["cur_pos"], 100);
    }

    #[test]
    fn test_serialize_settings_response() {
        let settings = SettingsPayload {
            midi_channel: Some(3),
            orientation: Some(1),
            led_brightness: Some(255),
            idle_timeout_s: Some(60),
        };
        let json = serialize_event(&settings_response(settings)).unwrap();
        assert!(json.contains("\"midi_channel\":3"));
        assert!(json.contains("\"led_brightness\":255"));
    }
}
