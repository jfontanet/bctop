use super::actions::{Action, Actions};

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
    pub fn get_actions(&self) -> Actions {
        if self.is_monitoring() {
            vec![
                Action::Quit,
                Action::ShowLogs,
                Action::ExecCommands,
                Action::Next,
                Action::Previous,
                Action::StopContainer,
            ]
            .into()
        } else if self.is_logging() {
            vec![
                Action::Quit,
                Action::ScrollDown,
                Action::ScrollUp,
                Action::Search,
                Action::Remove,
            ]
            .into()
        } else if self.is_exec_command() {
            vec![Action::Quit, Action::SendCMD].into()
        } else {
            vec![Action::Quit].into()
        }
    }

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
