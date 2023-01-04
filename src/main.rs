use bctop::app::App;
use bctop::io::handler::IoAsyncHandler;
use bctop::io::IoEvent;
use bctop::start_ui;
use eyre::Result;
use std::sync::Arc;
use tokio;

use log::LevelFilter;
use log4rs::append::file::FileAppender;
use log4rs::config::{Appender, Config, Root};
use log4rs::encode::pattern::PatternEncoder;
use reqwest;
use serde::Deserialize;
use std::error::Error;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let logfile = FileAppender::builder()
        .encoder(Box::new(PatternEncoder::new("{l} - {m}\n")))
        .build("/var/log/bctop.log")?;

    let config = Config::builder()
        .appender(Appender::builder().build("logfile", Box::new(logfile)))
        .build(Root::builder().appender("logfile").build(LevelFilter::Info))?;

    log4rs::init_config(config)?;

    let (sync_io_tx, mut sync_io_rx) = tokio::sync::mpsc::channel::<IoEvent>(100);
    let app = Arc::new(tokio::sync::Mutex::new(App::new(sync_io_tx.clone())));
    let app_ui = Arc::clone(&app);

    tokio::spawn(async move {
        let mut handler = IoAsyncHandler::new(app);
        while let Some(io_event) = sync_io_rx.recv().await {
            handler.handle_io_event(io_event).await;
        }
    });

    start_ui(&app_ui).await?;
    // Check for updates and print to stdout.
    println!("Checking for updates...");
    let cli = reqwest::Client::new();
    let resp = cli
        .get("https://api.github.com/repos/jfontanet/bctop/releases")
        .header(reqwest::header::USER_AGENT, "jfontanet")
        .send()
        .await?
        .json::<Vec<Release>>()
        .await?;

    let last_release = resp.iter().reduce(|a, b| {
        let mut a_versions = a.name.split(".");
        let a_major = a_versions
            .next()
            .unwrap()
            .split("v")
            .nth(1)
            .unwrap()
            .parse::<i32>()
            .unwrap();
        let a_minor = a_versions.next().unwrap().parse::<i32>().unwrap();
        let a_patch = a_versions.next().unwrap().parse::<i32>().unwrap();
        let mut b_versions = b.name.split(".");
        let b_major = b_versions
            .next()
            .unwrap()
            .split("v")
            .nth(1)
            .unwrap()
            .parse::<i32>()
            .unwrap();
        let b_minor = b_versions.next().unwrap().parse::<i32>().unwrap();
        let b_patch = b_versions.next().unwrap().parse::<i32>().unwrap();

        if a_major > b_major {
            a
        } else if a_major < b_major {
            b
        } else if a_minor > b_minor {
            a
        } else if a_minor < b_minor {
            b
        } else if a_patch > b_patch {
            a
        } else if a_patch < b_patch {
            b
        } else {
            a
        }
    });
    if let Some(last_release) = last_release {
        // asume that the new latest is major than the one installed.
        if last_release.name != format!("v{}", VERSION) {
            println!(
                "New version available: {} (from: {}) \n Download here: {}",
                last_release.name, VERSION, last_release.assets[0].browser_download_url
            );
            return Ok(());
        }
    }
    println!("No updates available.");
    Ok(())
}

#[derive(Debug, Deserialize)]
struct Release {
    name: String, // version
    assets: Vec<Asset>,
}

#[derive(Debug, Deserialize)]
struct Asset {
    // name: String,
    browser_download_url: String,
    // state: String,
}
