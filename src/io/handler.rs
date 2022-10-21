use eyre::Result;
// use log::{error, info};
use std::{sync::Arc, time::Duration};
use tokio::sync::Mutex;

use super::IoEvent;

use crate::app::container_management::start_management_process;
use crate::app::App;

pub struct IoAsyncHandler {
    app: Arc<Mutex<App>>,
}

impl IoAsyncHandler {
    pub fn new(app: Arc<tokio::sync::Mutex<App>>) -> Self {
        Self { app }
    }

    /// We could be async here
    pub async fn handle_io_event(&mut self, io_event: IoEvent) {
        let result = match io_event {
            IoEvent::Initialize => self.do_initialize().await,
            IoEvent::Sleep(duration) => self.do_sleep(duration).await,
        };

        if let Err(_err) = result {
            // error!("Oops, something wrong happen: {:?}", err);
        }

        let mut _app = self.app.lock().await;
    }

    /// We use dummy implementation here, just wait 1s
    async fn do_initialize(&mut self) -> Result<()> {
        // info!("🚀 Initialize the application");
        let mut app = self.app.lock().await;
        tokio::time::sleep(Duration::from_secs(1)).await;
        app.initialized().await;
        // info!("👍 Application initialized");
        start_management_process(Arc::clone(&self.app)).await;
        Ok(())
    }

    /// Just take a little break
    async fn do_sleep(&mut self, duration: Duration) -> Result<()> {
        // info!("😴 Go sleeping for {:?}...", duration);
        tokio::time::sleep(duration).await;
        // info!("⏰ Wake up !");
        // Notify the app for having slept
        let mut _app = self.app.lock().await;
        // app.slept();

        Ok(())
    }
}