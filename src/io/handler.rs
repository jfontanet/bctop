use eyre::Result;
use log::{error, info};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use super::IoEvent;

use crate::app::App;
use crate::container_management::{
    pause_container, start_management_process, start_monitoring_logs, stop_container,
};

pub struct IoAsyncHandler {
    app: Arc<Mutex<App>>,
    active_task: Option<JoinHandle<()>>,
}

impl IoAsyncHandler {
    pub fn new(app: Arc<tokio::sync::Mutex<App>>) -> Self {
        Self {
            app,
            active_task: None,
        }
    }

    /// We could be async here
    pub async fn handle_io_event(&mut self, io_event: IoEvent) {
        let result = match io_event {
            IoEvent::StartMonitoring => self.start_management().await,
            IoEvent::ShowLogs(container_id) => self.start_logs_monitoring(container_id).await,
            IoEvent::StopContainer(container_id) => self.stop_container(container_id).await,
            IoEvent::PauseContainer(container_id) => self.pause_container(container_id).await,
        };

        if let Err(err) = result {
            error!("Oops, something wrong happen: {:?}", err);
        }
    }

    async fn start_management(&mut self) -> Result<()> {
        self.abort_current_task().await;
        let app = Arc::clone(&self.app);
        let t = tokio::spawn(async move {
            start_management_process(app).await;
        });
        self.active_task = Some(t);
        Ok(())
    }

    async fn start_logs_monitoring(&mut self, container_id: String) -> Result<()> {
        self.abort_current_task().await;
        info!("Start monitoring logs for container: {}", container_id);
        let app = Arc::clone(&self.app);
        let t = tokio::spawn(async move {
            start_monitoring_logs(container_id, app).await;
        });
        self.active_task = Some(t);
        Ok(())
    }

    async fn abort_current_task(&mut self) {
        if let Some(task) = self.active_task.take() {
            task.abort();
            match task.await {
                Ok(_) => return,
                Err(_) => return,
            };
        }
    }

    async fn stop_container(&mut self, container_id: String) -> Result<()> {
        self.abort_current_task().await;
        info!("Stop container: {}", container_id);
        stop_container(container_id).await;
        Ok(())
    }

    async fn pause_container(&mut self, container_id: String) -> Result<()> {
        self.abort_current_task().await;
        info!("Pause container: {}", container_id);
        pause_container(container_id).await;
        Ok(())
    }
}
