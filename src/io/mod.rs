use tokio::sync::mpsc::Receiver;

pub mod handler;

#[derive(Debug)]
pub struct SessionObject {
    pub container_id: String,
    pub rx_channel: Receiver<String>,
}

#[derive(Debug)]
pub enum IoEvent {
    StartMonitoring,
    ShowLogs(String),
    StopContainer(String),
    PauseContainer(String),
    StartExecSession(SessionObject),
}
