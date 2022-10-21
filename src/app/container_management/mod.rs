use docker_api::opts::LogsOptsBuilder;
use futures::stream::StreamExt;
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tokio::{sync::Mutex, time};

#[derive(Deserialize, Debug)]
struct MemoryStats {
    #[serde(default)]
    limit: u64,
    #[serde(default)]
    usage: u64,
}
impl Default for MemoryStats {
    fn default() -> Self {
        MemoryStats { limit: 0, usage: 0 }
    }
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
struct CPUStats {
    cpu_usage: CPUUsage,
    #[serde(default)]
    online_cpus: u64,
    #[serde(default)]
    system_cpu_usage: u64,
}
impl Default for CPUStats {
    fn default() -> Self {
        CPUStats {
            cpu_usage: CPUUsage {
                total_usage: 0,
                usage_in_kernelmode: 0,
                usage_in_usermode: 0,
            },
            online_cpus: 0,
            system_cpu_usage: 0,
        }
    }
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
struct CPUUsage {
    total_usage: u64,
    usage_in_kernelmode: u64,
    usage_in_usermode: u64,
}

#[derive(Deserialize, Debug)]
struct ContainerStats {
    #[serde(default)]
    cpu_stats: CPUStats,
    memory_stats: MemoryStats,
}

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
}

pub async fn get_logs_from(
    from: &chrono::DateTime<chrono::Utc>,
    container_id: String,
) -> Vec<String> {
    let docker = docker_api::Docker::unix("/var/run/docker.sock");
    let container = docker.containers().get(&container_id);
    let log_ops = LogsOptsBuilder::default()
        .since(from)
        .follow(false)
        .stdout(true)
        .stderr(true)
        .n_lines(1000)
        .build();

    let mut logs = container.logs(&log_ops);
    let mut log_lines = Vec::new();
    while let Some(log) = logs.next().await {
        match log {
            Ok(log) => {
                let log_line = String::from_utf8(Vec::from(log)).unwrap();
                log_lines.push(log_line);
            }
            Err(e) => {
                println!("Error getting logs: {}", e);
            }
        }
    }
    log_lines
}

pub fn enter_tty(_container_id: String) {
    todo!()
    // Enter the container tty. Add attribute to get new commands from the UI.
}

pub async fn start_management_process(
    manager: Arc<Mutex<impl ContainerManagement + std::marker::Send + 'static>>,
) {
    tokio::spawn(async move {
        let docker = docker_api::Docker::unix("/var/run/docker.sock");
        let mut alive_container_ids = HashSet::new();
        loop {
            let mut tasks = Vec::new();

            let containers_summary = docker.containers().list(&Default::default()).await.unwrap();

            let container_ids: HashSet<String> = containers_summary
                .clone()
                .iter()
                .map(|item| item.id.as_ref().unwrap_or(&String::from("")).to_string())
                .collect();
            let contaienrs_to_remove = &alive_container_ids - &container_ids;
            for container_id in contaienrs_to_remove {
                manager.lock().await.remove_container(&container_id);
            }

            alive_container_ids = container_ids;

            for container_summary in containers_summary {
                let m = manager.clone();
                let cs = container_summary.clone();
                let t = tokio::spawn(async move {
                    update_container(cs, m).await;
                });
                tasks.push(t);
            }

            for t in tasks {
                t.await.unwrap();
            }
            time::sleep(time::Duration::from_secs(1)).await;
        }
    });
}

async fn update_container(
    container_summary: docker_api::models::ContainerSummary,
    manager: Arc<Mutex<impl ContainerManagement>>,
) {
    let docker = docker_api::Docker::unix("/var/run/docker.sock");
    let container_id = container_summary.id.unwrap_or_default();
    let container = docker.containers().get(&container_id);

    let inspect = container.inspect().await;
    if inspect.is_err() {
        return;
    }
    let inspect = inspect.unwrap();

    let m = container.stats().next().await;
    if m.is_none() {
        return;
    }
    let m = m.unwrap();
    if m.is_err() {
        return;
    }
    let m = m.unwrap();

    let stats: ContainerStats = serde_json::from_value(m).unwrap();
    let labels = container_summary.labels.unwrap_or(HashMap::new());

    let container = Container {
        id: container_id,
        status: ContainerStatus::from(
            inspect
                .state
                .unwrap()
                .status
                .unwrap_or(String::from("running")),
        ),
        name: inspect.name.unwrap(),
        image: inspect.image.unwrap(),
        cpu_usage: (stats.cpu_stats.cpu_usage.total_usage as f32
            / stats.cpu_stats.system_cpu_usage as f32)
            * 100.0,
        memory_usage_bytes: stats.memory_stats.usage as f32,
        memory_limit_bytes: stats.memory_stats.limit as f32,
        swarm_service: labels.get("com.docker.swarm.service.name").cloned(),
        swarm_stack: labels.get("com.docker.stack.namespace").cloned(),
        compose_service: labels.get("com.docker.compose.service").cloned(),
        compose_project: labels.get("com.docker.compose.project").cloned(),
    };
    manager.lock().await.update_containers(container);
}
