mod docker;

pub use docker::{
    enter_tty, pause_container, start_management_process, start_monitoring_logs, stop_container,
};

#[derive(Debug, Clone)]
pub struct Container {
    pub id: String,
    pub status: ContainerStatus,
    pub name: String,
    pub image: String,
    pub cpu_usage: f32,
    pub memory_usage_bytes: f32,
    pub memory_limit_bytes: f32,
    pub swarm_service: Option<String>,
    pub swarm_stack: Option<String>,
    pub compose_service: Option<String>,
    pub compose_project: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ContainerStatus {
    Created,
    Running,
    Paused,
    Stopped,
    Restarting,
    Removing,
    Exited,
    Dead,
}

impl From<String> for ContainerStatus {
    fn from(s: String) -> Self {
        match s.as_str() {
            "created" => ContainerStatus::Created,
            "running" => ContainerStatus::Running,
            "paused" => ContainerStatus::Paused,
            "stopped" => ContainerStatus::Stopped,
            "restarting" => ContainerStatus::Restarting,
            "removing" => ContainerStatus::Removing,
            "exited" => ContainerStatus::Exited,
            "dead" => ContainerStatus::Dead,
            _ => ContainerStatus::Running,
        }
    }
}

pub trait ContainerManagement {
    fn remove_container(&mut self, id: &str);
    fn update_containers(&mut self, new_container: Container);
    fn add_logs(&mut self, logs: Vec<String>);
}
