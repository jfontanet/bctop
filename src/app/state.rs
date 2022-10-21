#[derive(Clone)]
pub enum AppState {
    Monitoring,
    Logging { container: String },
    ExecCommand { container: String },
}

impl Default for AppState {
    fn default() -> Self {
        Self::Monitoring
    }
}

impl AppState {
    pub fn is_monitoring(&self) -> bool {
        matches!(self, &Self::Monitoring)
    }

    pub fn is_logging(&self) -> bool {
        matches!(self, &Self::Logging { .. })
    }

    pub fn is_exec_command(&self) -> bool {
        matches!(self, &Self::ExecCommand { .. })
    }
}
