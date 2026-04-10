use super::command::*;

/// Parse a JSON string into a Command.
pub fn parse_command(json: &str) -> Result<Command, ParseError> {
    let val: serde_json::Value =
        serde_json::from_str(json).map_err(|e| ParseError::InvalidJson(e.to_string()))?;

    let obj = val
        .as_object()
        .ok_or_else(|| ParseError::InvalidFormat("expected JSON object".into()))?;

    // Route by top-level key
    if let Some(v) = obj.get("profile") {
        let payload: ProfilePayload =
            serde_json::from_value(v.clone()).map_err(|e| ParseError::InvalidField(e.to_string()))?;
        return Ok(Command::SetProfile(payload));
    }

    if let Some(v) = obj.get("settings") {
        let payload: SettingsPayload =
            serde_json::from_value(v.clone()).map_err(|e| ParseError::InvalidField(e.to_string()))?;
        return Ok(Command::SetSettings(payload));
    }

    if let Some(v) = obj.get("motor") {
        let cmd: MotorCommand =
            serde_json::from_value(v.clone()).map_err(|e| ParseError::InvalidField(e.to_string()))?;
        return Ok(Command::Motor(cmd));
    }

    if let Some(v) = obj.get("screen") {
        let cmd: ScreenCommand =
            serde_json::from_value(v.clone()).map_err(|e| ParseError::InvalidField(e.to_string()))?;
        return Ok(Command::Screen(cmd));
    }

    if obj.get("save").is_some() {
        return Ok(Command::Save);
    }

    if let Some(v) = obj.get("load") {
        if let Some(load_obj) = v.as_object() {
            if let Some(name) = load_obj.get("name").and_then(|n| n.as_str()) {
                return Ok(Command::Load(name.to_string()));
            }
        }
        // Also accept {"load": "name"}
        if let Some(name) = v.as_str() {
            return Ok(Command::Load(name.to_string()));
        }
        return Err(ParseError::InvalidField("load requires a name".into()));
    }

    if obj.get("list").is_some() {
        return Ok(Command::List);
    }

    if obj.get("get").is_some() {
        return Ok(Command::Get);
    }

    if obj.get("get_settings").is_some() {
        return Ok(Command::GetSettings);
    }

    Err(ParseError::UnknownCommand)
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParseError {
    InvalidJson(String),
    InvalidFormat(String),
    InvalidField(String),
    UnknownCommand,
}

impl core::fmt::Display for ParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ParseError::InvalidJson(e) => write!(f, "invalid JSON: {e}"),
            ParseError::InvalidFormat(e) => write!(f, "invalid format: {e}"),
            ParseError::InvalidField(e) => write!(f, "invalid field: {e}"),
            ParseError::UnknownCommand => write!(f, "unknown command"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_get() {
        let cmd = parse_command(r#"{"get": true}"#).unwrap();
        assert_eq!(cmd, Command::Get);
    }

    #[test]
    fn test_parse_list() {
        let cmd = parse_command(r#"{"list": true}"#).unwrap();
        assert_eq!(cmd, Command::List);
    }

    #[test]
    fn test_parse_save() {
        let cmd = parse_command(r#"{"save": true}"#).unwrap();
        assert_eq!(cmd, Command::Save);
    }

    #[test]
    fn test_parse_load_object() {
        let cmd = parse_command(r#"{"load": {"name": "my_profile"}}"#).unwrap();
        assert_eq!(cmd, Command::Load("my_profile".into()));
    }

    #[test]
    fn test_parse_load_string() {
        let cmd = parse_command(r#"{"load": "my_profile"}"#).unwrap();
        assert_eq!(cmd, Command::Load("my_profile".into()));
    }

    #[test]
    fn test_parse_motor_recalibrate() {
        let cmd = parse_command(r#"{"motor": {"recalibrate": true}}"#).unwrap();
        assert_eq!(
            cmd,
            Command::Motor(MotorCommand { recalibrate: true })
        );
    }

    #[test]
    fn test_parse_settings() {
        let cmd =
            parse_command(r#"{"settings": {"midi_channel": 2, "led_brightness": 128}}"#).unwrap();
        match cmd {
            Command::SetSettings(s) => {
                assert_eq!(s.midi_channel, Some(2));
                assert_eq!(s.led_brightness, Some(128));
                assert_eq!(s.orientation, None);
            }
            _ => panic!("expected SetSettings"),
        }
    }

    #[test]
    fn test_parse_profile() {
        let json = r#"{"profile": {"name": "test", "haptic": {"mode": "regular", "start_pos": 0, "end_pos": 255, "detent_count": 60, "vernier": 5, "kx_force": false, "output_ramp": 5000.0, "detent_strength": 3.0}}}"#;
        let cmd = parse_command(json).unwrap();
        match cmd {
            Command::SetProfile(p) => {
                assert_eq!(p.name, "test");
                let h = p.haptic.unwrap();
                assert_eq!(h.detent_count, 60);
                assert_eq!(h.mode, "regular");
            }
            _ => panic!("expected SetProfile"),
        }
    }

    #[test]
    fn test_parse_profile_minimal() {
        let cmd = parse_command(r#"{"profile": {"name": "bare"}}"#).unwrap();
        match cmd {
            Command::SetProfile(p) => {
                assert_eq!(p.name, "bare");
                assert!(p.haptic.is_none());
            }
            _ => panic!("expected SetProfile"),
        }
    }

    #[test]
    fn test_parse_invalid_json() {
        let result = parse_command("not json");
        assert!(matches!(result, Err(ParseError::InvalidJson(_))));
    }

    #[test]
    fn test_parse_unknown_command() {
        let result = parse_command(r#"{"foo": "bar"}"#);
        assert_eq!(result, Err(ParseError::UnknownCommand));
    }

    #[test]
    fn test_parse_non_object() {
        let result = parse_command(r#"[1, 2, 3]"#);
        assert!(matches!(result, Err(ParseError::InvalidFormat(_))));
    }

    #[test]
    fn test_parse_get_settings() {
        let cmd = parse_command(r#"{"get_settings": true}"#).unwrap();
        assert_eq!(cmd, Command::GetSettings);
    }
}
