use super::Orchestrator;
use super::timestamp_now;
use super::types::LegacyFile;
use crate::core::lock_utils::*;

impl Orchestrator {
    pub fn protect_legacy_file(&self, path: &str, reason: &str) {
        let mut files = self.legacy_files.lock_safe();
        files.insert(
            path.to_string(),
            LegacyFile {
                path: path.to_string(),
                protected: true,
                locked_by: None,
                reason: reason.to_string(),
                timestamp: timestamp_now(),
            },
        );
    }

    pub fn lock_legacy_file(&self, path: &str, agent_id: &str) -> bool {
        let mut files = self.legacy_files.lock_safe();
        if let Some(file) = files.get_mut(path) {
            if file.locked_by.is_some() {
                return false;
            }
            file.locked_by = Some(agent_id.to_string());
            true
        } else {
            false
        }
    }

    pub fn unlock_legacy_file(&self, path: &str, agent_id: &str) -> bool {
        let mut files = self.legacy_files.lock_safe();
        if let Some(file) = files.get_mut(path) {
            if file.locked_by.as_deref() == Some(agent_id) {
                file.locked_by = None;
                return true;
            }
        }
        false
    }

    pub fn can_modify_legacy(&self, path: &str, agent_id: &str) -> Result<(), String> {
        let files = self.legacy_files.lock_safe();
        if let Some(file) = files.get(path) {
            if !file.protected {
                return Ok(());
            }
            if let Some(ref locker) = file.locked_by {
                if locker != agent_id {
                    return Err(format!("File '{}' locked by {}", path, locker));
                }
            } else {
                return Err(format!(
                    "File '{}' is legacy-protected. Request lock first.",
                    path
                ));
            }
        }
        Ok(())
    }

    pub fn get_legacy_files(&self) -> Vec<LegacyFile> {
        self.legacy_files.lock_safe().values().cloned().collect()
    }
}
