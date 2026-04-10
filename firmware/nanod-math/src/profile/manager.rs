use crate::protocol::command::ProfilePayload;

const MAX_PROFILES: usize = 10;

/// In-memory profile store. Manages up to 10 profiles with dirty tracking.
/// File I/O (SPIFFS) is handled by the firmware crate — this is pure logic.
pub struct ProfileManager {
    profiles: Vec<ProfileEntry>,
    active_index: Option<usize>,
}

#[derive(Debug, Clone)]
struct ProfileEntry {
    payload: ProfilePayload,
    dirty: bool,
}

impl ProfileManager {
    pub fn new() -> Self {
        Self {
            profiles: Vec::new(),
            active_index: None,
        }
    }

    /// Add or update a profile. Returns true if a new profile was created.
    pub fn set_profile(&mut self, payload: ProfilePayload) -> Result<bool, ManagerError> {
        // Update existing
        if let Some(idx) = self.find_index(&payload.name) {
            self.profiles[idx].payload = payload;
            self.profiles[idx].dirty = true;
            return Ok(false);
        }

        // Add new
        if self.profiles.len() >= MAX_PROFILES {
            return Err(ManagerError::Full);
        }
        self.profiles.push(ProfileEntry {
            payload,
            dirty: true,
        });
        Ok(true)
    }

    /// Get a profile by name.
    pub fn get_profile(&self, name: &str) -> Option<&ProfilePayload> {
        self.find_index(name).map(|i| &self.profiles[i].payload)
    }

    /// Set the active profile by name. Returns error if not found.
    pub fn set_active(&mut self, name: &str) -> Result<(), ManagerError> {
        let idx = self.find_index(name).ok_or(ManagerError::NotFound)?;
        self.active_index = Some(idx);
        Ok(())
    }

    /// Get the currently active profile.
    pub fn active_profile(&self) -> Option<&ProfilePayload> {
        self.active_index.map(|i| &self.profiles[i].payload)
    }

    /// Get the name of the active profile.
    pub fn active_name(&self) -> Option<&str> {
        self.active_index
            .map(|i| self.profiles[i].payload.name.as_str())
    }

    /// List all profile names.
    pub fn list_names(&self) -> Vec<String> {
        self.profiles.iter().map(|e| e.payload.name.clone()).collect()
    }

    /// Remove a profile by name.
    pub fn remove(&mut self, name: &str) -> Result<(), ManagerError> {
        let idx = self.find_index(name).ok_or(ManagerError::NotFound)?;
        self.profiles.remove(idx);
        // Fix active index
        match self.active_index {
            Some(i) if i == idx => self.active_index = None,
            Some(i) if i > idx => self.active_index = Some(i - 1),
            _ => {}
        }
        Ok(())
    }

    /// Get all profiles that have been modified since last save.
    pub fn dirty_profiles(&self) -> Vec<&ProfilePayload> {
        self.profiles
            .iter()
            .filter(|e| e.dirty)
            .map(|e| &e.payload)
            .collect()
    }

    /// Mark all profiles as clean (after saving).
    pub fn mark_all_clean(&mut self) {
        for entry in &mut self.profiles {
            entry.dirty = false;
        }
    }

    /// Mark a specific profile as clean.
    pub fn mark_clean(&mut self, name: &str) {
        if let Some(idx) = self.find_index(name) {
            self.profiles[idx].dirty = false;
        }
    }

    /// Load a profile from external storage (e.g. SPIFFS).
    /// Marks it as clean since it just came from disk.
    pub fn load_from_storage(&mut self, payload: ProfilePayload) -> Result<(), ManagerError> {
        if let Some(idx) = self.find_index(&payload.name) {
            self.profiles[idx].payload = payload;
            self.profiles[idx].dirty = false;
            return Ok(());
        }
        if self.profiles.len() >= MAX_PROFILES {
            return Err(ManagerError::Full);
        }
        self.profiles.push(ProfileEntry {
            payload,
            dirty: false,
        });
        Ok(())
    }

    /// Number of stored profiles.
    pub fn count(&self) -> usize {
        self.profiles.len()
    }

    fn find_index(&self, name: &str) -> Option<usize> {
        self.profiles.iter().position(|e| e.payload.name == name)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ManagerError {
    Full,
    NotFound,
}

impl core::fmt::Display for ManagerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ManagerError::Full => write!(f, "profile storage full (max {MAX_PROFILES})"),
            ManagerError::NotFound => write!(f, "profile not found"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_profile(name: &str) -> ProfilePayload {
        ProfilePayload {
            name: name.into(),
            haptic: None,
        }
    }

    #[test]
    fn test_add_and_get() {
        let mut mgr = ProfileManager::new();
        mgr.set_profile(make_profile("a")).unwrap();
        assert!(mgr.get_profile("a").is_some());
        assert!(mgr.get_profile("b").is_none());
    }

    #[test]
    fn test_update_existing() {
        let mut mgr = ProfileManager::new();
        assert!(mgr.set_profile(make_profile("a")).unwrap()); // new
        assert!(!mgr.set_profile(make_profile("a")).unwrap()); // update
        assert_eq!(mgr.count(), 1);
    }

    #[test]
    fn test_max_profiles() {
        let mut mgr = ProfileManager::new();
        for i in 0..10 {
            mgr.set_profile(make_profile(&format!("p{i}"))).unwrap();
        }
        assert_eq!(mgr.set_profile(make_profile("overflow")), Err(ManagerError::Full));
    }

    #[test]
    fn test_list_names() {
        let mut mgr = ProfileManager::new();
        mgr.set_profile(make_profile("alpha")).unwrap();
        mgr.set_profile(make_profile("beta")).unwrap();
        mgr.set_profile(make_profile("gamma")).unwrap();
        let names = mgr.list_names();
        assert_eq!(names, vec!["alpha", "beta", "gamma"]);
    }

    #[test]
    fn test_active_profile() {
        let mut mgr = ProfileManager::new();
        mgr.set_profile(make_profile("a")).unwrap();
        mgr.set_profile(make_profile("b")).unwrap();
        assert!(mgr.active_profile().is_none());

        mgr.set_active("b").unwrap();
        assert_eq!(mgr.active_name(), Some("b"));
    }

    #[test]
    fn test_set_active_not_found() {
        let mut mgr = ProfileManager::new();
        assert_eq!(mgr.set_active("nope"), Err(ManagerError::NotFound));
    }

    #[test]
    fn test_remove() {
        let mut mgr = ProfileManager::new();
        mgr.set_profile(make_profile("a")).unwrap();
        mgr.set_profile(make_profile("b")).unwrap();
        mgr.set_active("b").unwrap();

        mgr.remove("a").unwrap();
        assert_eq!(mgr.count(), 1);
        assert_eq!(mgr.active_name(), Some("b")); // active adjusted
    }

    #[test]
    fn test_remove_active_clears() {
        let mut mgr = ProfileManager::new();
        mgr.set_profile(make_profile("a")).unwrap();
        mgr.set_active("a").unwrap();
        mgr.remove("a").unwrap();
        assert!(mgr.active_profile().is_none());
    }

    #[test]
    fn test_dirty_tracking() {
        let mut mgr = ProfileManager::new();
        mgr.set_profile(make_profile("a")).unwrap();
        mgr.set_profile(make_profile("b")).unwrap();
        assert_eq!(mgr.dirty_profiles().len(), 2);

        mgr.mark_clean("a");
        assert_eq!(mgr.dirty_profiles().len(), 1);
        assert_eq!(mgr.dirty_profiles()[0].name, "b");

        mgr.mark_all_clean();
        assert_eq!(mgr.dirty_profiles().len(), 0);
    }

    #[test]
    fn test_load_from_storage_is_clean() {
        let mut mgr = ProfileManager::new();
        mgr.load_from_storage(make_profile("disk")).unwrap();
        assert_eq!(mgr.dirty_profiles().len(), 0);
        assert_eq!(mgr.count(), 1);
    }

    #[test]
    fn test_remove_not_found() {
        let mut mgr = ProfileManager::new();
        assert_eq!(mgr.remove("nope"), Err(ManagerError::NotFound));
    }

    #[test]
    fn test_serialization_roundtrip() {
        let profile = ProfilePayload {
            name: "roundtrip".into(),
            haptic: Some(crate::protocol::command::HapticConfig {
                mode: "vernier".into(),
                start_pos: 0,
                end_pos: 100,
                detent_count: 20,
                vernier: 5,
                kx_force: true,
                output_ramp: 5000.0,
                detent_strength: 3.0,
            }),
        };
        let json = serde_json::to_string(&profile).unwrap();
        let parsed: ProfilePayload = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "roundtrip");
        assert_eq!(parsed.haptic.unwrap().vernier, 5);
    }
}
