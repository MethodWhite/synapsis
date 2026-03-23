//! Passive Capture Module

pub struct PassiveCapture {
    enabled: bool,
}

impl PassiveCapture {
    pub fn new() -> Self {
        Self { enabled: true }
    }
    pub fn enable(&mut self) {
        self.enabled = true;
    }
    pub fn disable(&mut self) {
        self.enabled = false;
    }
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
    pub fn extract_from_command(&self, _command: &str) -> Option<crate::domain::Observation> {
        if !self.enabled {
            return None;
        }
        None
    }
    pub fn extract_from_output(&self, _output: &str) -> Option<crate::domain::Observation> {
        if !self.enabled {
            return None;
        }
        None
    }
}

impl Default for PassiveCapture {
    fn default() -> Self {
        Self::new()
    }
}
