pub mod handler;

#[derive(Debug)]
pub enum IoEvent {
    StartMonitoring,
    ShowLogs(String),
    StopContainer(String),
    PauseContainer(String),
}
