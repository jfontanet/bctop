use std::collections::{HashMap, HashSet};
use std::panic;
use std::sync::Arc;

use bollard::container::{ListContainersOptions, LogsOptions, StatsOptions};
use bollard::Docker;

use bollard::exec::{CreateExecOptions, StartExecResults};
use bollard::service::ContainerSummary;
use chrono::TimeZone;
use chrono::Utc;
use futures::stream::StreamExt;
use log::error;
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
        _ => return,
    };
    let cpu_container_usage =
        stats.cpu_stats.cpu_usage.total_usage - stats.precpu_stats.cpu_usage.total_usage;
    let csu = match stats.cpu_stats.system_cpu_usage {
        Some(s) => s,
        None => return,
    };
    let psu = match stats.precpu_stats.system_cpu_usage {
        Some(s) => s,
        None => return,
    };
    let cpu_system_usage = csu - psu;
    let cpu_usage = cpu_container_usage as f32 / cpu_system_usage as f32
        * 100.0
        * stats.cpu_stats.online_cpus.unwrap_or(1) as f32;

    let memory_usage = stats.memory_stats.usage.unwrap() as f32;
    let memory_limit = stats.memory_stats.limit.unwrap() as f32;

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

pub async fn enter_tty(container_id: String) {
    let docker = Docker::connect_with_local_defaults().unwrap();
    let exec = docker
        .create_exec(
            &container_id,
            CreateExecOptions {
                attach_stdout: Some(true),
                attach_stderr: Some(true),
                attach_stdin: Some(true),
                tty: Some(true),
                cmd: Some(vec!["/bin/bash".to_string()]),
                ..Default::default()
            },
        )
        .await
        .unwrap()
        .id;

    // if let StartExecResults::Attached {
    //     mut output,
    //     mut input,
    // } = docker.start_exec(&exec, None).await.unwrap()
    // {}
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
