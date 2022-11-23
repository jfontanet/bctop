pub mod handler;

#[derive(Debug, Clone)]
pub enum IoEvent {
    StartMonitoring,
    ShowLogs(String),
    StopContainer(String),
    PauseContainer(String),
}
