use std::collections::{HashMap, HashSet};
use std::panic;
use std::sync::Arc;

use bollard::container::{
    ListContainersOptions, LogsOptions, RemoveContainerOptions, StatsOptions, StopContainerOptions,
};
use bollard::Docker;

use bollard::service::{ContainerStateStatusEnum, ContainerSummary};
use chrono::TimeZone;
use chrono::Utc;
use futures::stream::StreamExt;
use log::{debug, error, info, warn};
use tokio::sync::Mutex;

use super::{Container, ContainerManagement, ContainerStatus};

pub async fn start_management_process(
    manager: Arc<Mutex<impl ContainerManagement + std::marker::Send + 'static>>,
) {
    let docker = Docker::connect_with_local_defaults().unwrap();
    let mut alive_container_ids = HashSet::new();
    loop {
        let mut tasks = Vec::new();

        let containers_summary = docker
            .list_containers(Some(ListContainersOptions::<String> {
                all: true,
                ..Default::default()
            }))
            .await
            .unwrap();
        let container_ids: HashSet<String> = containers_summary
            .clone()
            .iter()
            .map(|item| item.id.as_ref().unwrap_or(&String::from("")).to_string())
            .collect();
        let contaienrs_to_remove = &alive_container_ids - &container_ids;
        info!("Containers to remove: {:?}", contaienrs_to_remove);
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
            match t.await {
                Ok(_) => {}
                Err(e) => {
                    error!("Error updating container: {}", e);
                    if e.is_panic() {
                        panic::resume_unwind(e.into_panic());
                    }
                }
            };
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
}

async fn update_container(
    container_summary: ContainerSummary,
    manager: Arc<Mutex<impl ContainerManagement>>,
) {
    let docker = Docker::connect_with_local_defaults().unwrap();
    let container_id = container_summary.id.unwrap();
    let labels = container_summary.labels.unwrap_or(HashMap::new());

    debug!("Updating container: {}", container_id);

    let stream = &mut docker
        .stats(
            &container_id,
            Some(StatsOptions {
                stream: false,
                ..Default::default()
            }),
        )
        .take(1);
    let stats = match stream.next().await {
        Some(Ok(s)) => s,
        _ => {
            error!("Error getting stats for container: {}", container_id);
            return;
        }
    };

    let cpu_container_usage = stats
        .cpu_stats
        .cpu_usage
        .total_usage
        .checked_sub(stats.precpu_stats.cpu_usage.total_usage)
        .unwrap_or(0u64);
    let csu = stats.cpu_stats.system_cpu_usage.unwrap_or(0);
    let psu = stats.precpu_stats.system_cpu_usage.unwrap_or(0);
    let cpu_system_usage = csu - psu;
    let cpu_usage = if cpu_system_usage > 0 {
        cpu_container_usage as f32 / cpu_system_usage as f32
            * 100.0
            * stats.cpu_stats.online_cpus.unwrap_or(1) as f32
    } else {
        0.0
    };

    let memory_usage = stats.memory_stats.usage.unwrap_or(0) as f32;
    let memory_limit = stats.memory_stats.limit.unwrap_or(0) as f32;

    let container = Container {
        id: container_id,
        name: container_summary.names.unwrap()[0]
            .clone()
            .split("/")
            .last()
            .unwrap()
            .to_string(),
        image: container_summary.image.unwrap(),
        status: ContainerStatus::from(container_summary.state.unwrap_or(String::from("running"))),
        swarm_service: labels.get("com.docker.swarm.service.name").cloned(),
        swarm_stack: labels.get("com.docker.stack.namespace").cloned(),
        compose_service: labels.get("com.docker.compose.service").cloned(),
        compose_project: labels.get("com.docker.compose.project").cloned(),
        cpu_usage: cpu_usage,
        memory_usage_bytes: memory_usage,
        memory_limit_bytes: memory_limit,
    };

    manager.lock().await.update_containers(container);
}

pub async fn start_monitoring_logs(
    container_id: String,
    manager: Arc<Mutex<impl ContainerManagement + std::marker::Send + 'static>>,
) {
    let docker = Docker::connect_with_local_defaults().unwrap();
    let mut now = Utc.timestamp(0, 0);

    loop {
        let mut logs = docker.logs(
            &container_id,
            Some(LogsOptions {
                since: now.timestamp(),
                follow: false,
                stdout: true,
                stderr: true,
                tail: "all",
                ..Default::default()
            }),
        );
        let mut logs_vec = Vec::new();
        while let Some(Ok(chunk)) = logs.next().await {
            logs_vec.push(format!("{}", chunk));
        }
        now = Utc::now();
        manager.lock().await.add_logs(logs_vec);
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
}

pub async fn stop_container(container_id: String) {
    let docker = Docker::connect_with_local_defaults().unwrap();
    match docker.inspect_container(&container_id, None).await {
        Ok(container) => {
            let status = container
                .state
                .unwrap_or_default()
                .status
                .unwrap_or(ContainerStateStatusEnum::EMPTY);
            match status {
                ContainerStateStatusEnum::RUNNING => {
                    docker
                        .stop_container(
                            &container_id,
                            Some(StopContainerOptions {
                                t: 10,
                                ..Default::default()
                            }),
                        )
                        .await
                        .unwrap();
                }
                ContainerStateStatusEnum::EXITED | ContainerStateStatusEnum::CREATED => {
                    docker
                        .remove_container(
                            &container_id,
                            Some(RemoveContainerOptions {
                                force: true,
                                ..Default::default()
                            }),
                        )
                        .await
                        .unwrap();
                }
                _ => warn!("Container in invalid status: {}", status),
            }
        }
        Err(e) => {
            error!("Error stopping container: {}", e);
        }
    }
}

pub async fn pause_container(container_id: String) {
    let docker = Docker::connect_with_local_defaults().unwrap();
    match docker.inspect_container(&container_id, None).await {
        Ok(container) => {
            let status = container
                .state
                .unwrap_or_default()
                .status
                .unwrap_or(ContainerStateStatusEnum::EMPTY);
            if status == ContainerStateStatusEnum::RUNNING {
                docker.pause_container(&container_id).await.unwrap();
            } else if status == ContainerStateStatusEnum::PAUSED {
                docker.unpause_container(&container_id).await.unwrap();
            } else {
                debug!("Container is not running or paused");
            }
        }
        Err(e) => {
            error!("Error pausing container: {}", e);
        }
    };
}
