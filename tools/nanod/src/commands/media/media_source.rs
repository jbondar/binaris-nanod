use anyhow::Result;

/// Now Playing track information.
#[derive(Debug, Clone, PartialEq)]
pub struct NowPlaying {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_s: u32,
    pub position_s: u32,
    pub playing: bool,
    pub artwork_url: Option<String>,
}

/// Cross-platform media source trait.
pub trait MediaSource {
    fn get_now_playing(&mut self) -> Result<Option<NowPlaying>>;
    fn play_pause(&mut self) -> Result<()>;
    fn next_track(&mut self) -> Result<()>;
    fn prev_track(&mut self) -> Result<()>;
    fn set_volume(&mut self, percent: f32) -> Result<()>;
    fn seek_to(&mut self, position_s: u32) -> Result<()>;
}

/// macOS implementation using osascript.
#[cfg(target_os = "macos")]
pub struct MacOsSource {
    last_app: Option<String>,
}

#[cfg(target_os = "macos")]
impl MacOsSource {
    pub fn new() -> Self {
        Self { last_app: None }
    }

    fn detect_media_app(&mut self) -> Option<&str> {
        // Check common media apps in order of preference
        for app in &["Spotify", "Plexamp", "Music"] {
            if let Ok(output) = std::process::Command::new("osascript")
                .args(["-e", &format!(
                    "tell application \"System Events\" to (name of processes) contains \"{}\"",
                    app
                )])
                .output()
            {
                let result = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if result == "true" {
                    self.last_app = Some(app.to_string());
                    return self.last_app.as_deref();
                }
            }
        }
        None
    }

    fn osascript(&self, script: &str) -> Result<String> {
        let output = std::process::Command::new("osascript")
            .args(["-e", script])
            .output()?;
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}

#[cfg(target_os = "macos")]
impl MediaSource for MacOsSource {
    fn get_now_playing(&mut self) -> Result<Option<NowPlaying>> {
        let app = match self.detect_media_app() {
            Some(a) => a.to_string(),
            None => return Ok(None),
        };

        match app.as_str() {
            "Spotify" => {
                let script = r#"
                    tell application "Spotify"
                        if player state is playing or player state is paused then
                            set t to name of current track
                            set a to artist of current track
                            set al to album of current track
                            set d to (duration of current track) / 1000
                            set p to player position as integer
                            set s to player state as string
                            set art to artwork url of current track
                            return t & "|" & a & "|" & al & "|" & d & "|" & p & "|" & s & "|" & art
                        end if
                    end tell
                "#;
                let result = self.osascript(script)?;
                if result.is_empty() {
                    return Ok(None);
                }
                let parts: Vec<&str> = result.splitn(7, '|').collect();
                if parts.len() < 6 {
                    return Ok(None);
                }
                Ok(Some(NowPlaying {
                    title: parts[0].to_string(),
                    artist: parts[1].to_string(),
                    album: parts[2].to_string(),
                    duration_s: parts[3].parse().unwrap_or(0),
                    position_s: parts[4].parse().unwrap_or(0),
                    playing: parts[5] == "playing",
                    artwork_url: parts.get(6).map(|s| s.to_string()),
                }))
            }
            "Music" => {
                let script = r#"
                    tell application "Music"
                        if player state is playing or player state is paused then
                            set t to name of current track
                            set a to artist of current track
                            set al to album of current track
                            set d to duration of current track as integer
                            set p to player position as integer
                            set s to player state as string
                            return t & "|" & a & "|" & al & "|" & d & "|" & p & "|" & s
                        end if
                    end tell
                "#;
                let result = self.osascript(script)?;
                if result.is_empty() {
                    return Ok(None);
                }
                let parts: Vec<&str> = result.splitn(6, '|').collect();
                if parts.len() < 6 {
                    return Ok(None);
                }
                Ok(Some(NowPlaying {
                    title: parts[0].to_string(),
                    artist: parts[1].to_string(),
                    album: parts[2].to_string(),
                    duration_s: parts[3].parse().unwrap_or(0),
                    position_s: parts[4].parse().unwrap_or(0),
                    playing: parts[5] == "playing",
                    artwork_url: None,
                }))
            }
            _ => Ok(None),
        }
    }

    fn play_pause(&mut self) -> Result<()> {
        if let Some(app) = &self.last_app {
            self.osascript(&format!(
                "tell application \"{}\" to playpause", app
            ))?;
        }
        Ok(())
    }

    fn next_track(&mut self) -> Result<()> {
        if let Some(app) = &self.last_app {
            self.osascript(&format!(
                "tell application \"{}\" to next track", app
            ))?;
        }
        Ok(())
    }

    fn prev_track(&mut self) -> Result<()> {
        if let Some(app) = &self.last_app {
            self.osascript(&format!(
                "tell application \"{}\" to previous track", app
            ))?;
        }
        Ok(())
    }

    fn set_volume(&mut self, percent: f32) -> Result<()> {
        // Use system volume via osascript
        let vol = (percent.clamp(0.0, 100.0) / 100.0 * 7.0) as u8; // macOS volume 0-7
        self.osascript(&format!(
            "set volume output volume {}", (percent.clamp(0.0, 100.0) as u32)
        ))?;
        let _ = vol;
        Ok(())
    }

    fn seek_to(&mut self, position_s: u32) -> Result<()> {
        if let Some(app) = &self.last_app {
            self.osascript(&format!(
                "tell application \"{}\" to set player position to {}",
                app, position_s
            ))?;
        }
        Ok(())
    }
}

// Stubs for other platforms
#[cfg(target_os = "linux")]
pub struct LinuxSource;

#[cfg(target_os = "linux")]
impl LinuxSource {
    pub fn new() -> Self { Self }
}

#[cfg(target_os = "linux")]
impl MediaSource for LinuxSource {
    fn get_now_playing(&mut self) -> Result<Option<NowPlaying>> {
        anyhow::bail!("Linux MPRIS integration not yet implemented")
    }
    fn play_pause(&mut self) -> Result<()> { anyhow::bail!("not implemented") }
    fn next_track(&mut self) -> Result<()> { anyhow::bail!("not implemented") }
    fn prev_track(&mut self) -> Result<()> { anyhow::bail!("not implemented") }
    fn set_volume(&mut self, _: f32) -> Result<()> { anyhow::bail!("not implemented") }
    fn seek_to(&mut self, _: u32) -> Result<()> { anyhow::bail!("not implemented") }
}

#[cfg(target_os = "windows")]
pub struct WindowsSource;

#[cfg(target_os = "windows")]
impl WindowsSource {
    pub fn new() -> Self { Self }
}

#[cfg(target_os = "windows")]
impl MediaSource for WindowsSource {
    fn get_now_playing(&mut self) -> Result<Option<NowPlaying>> {
        anyhow::bail!("Windows SMTC integration not yet implemented")
    }
    fn play_pause(&mut self) -> Result<()> { anyhow::bail!("not implemented") }
    fn next_track(&mut self) -> Result<()> { anyhow::bail!("not implemented") }
    fn prev_track(&mut self) -> Result<()> { anyhow::bail!("not implemented") }
    fn set_volume(&mut self, _: f32) -> Result<()> { anyhow::bail!("not implemented") }
    fn seek_to(&mut self, _: u32) -> Result<()> { anyhow::bail!("not implemented") }
}

/// Create the platform-appropriate media source.
pub fn create_media_source() -> Box<dyn MediaSource> {
    #[cfg(target_os = "macos")]
    { Box::new(MacOsSource::new()) }
    #[cfg(target_os = "linux")]
    { Box::new(LinuxSource::new()) }
    #[cfg(target_os = "windows")]
    { Box::new(WindowsSource::new()) }
}
