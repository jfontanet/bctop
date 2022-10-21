pub mod app;
pub mod inputs;
pub mod io;

use app::{ui, App, AppReturn};
use eyre::Result;
use inputs::{events::Events, InputEvent};
use io::IoEvent;
use std::{io::stdout, sync::Arc, time::Duration};

pub async fn start_ui(app: &Arc<tokio::sync::Mutex<App>>) -> Result<()> {
    let mut stdout = stdout();
    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    let backend = tui::backend::CrosstermBackend::new(stdout);
    let mut terminal = tui::Terminal::new(backend)?;
    terminal.clear()?;
    terminal.hide_cursor()?;

    let tick_rate = Duration::from_millis(200);
    let mut events = Events::new(tick_rate);

    // Trigger state change from Init to Initialized
    {
        let mut app = app.lock().await;
        // Here we assume the the first load is a long task
        app.dispatch(IoEvent::Initialize).await;
    }

    loop {
        let mut app = app.lock().await;
        terminal.draw(|rect| ui::draw(rect, &mut app))?;

        let result = match events.next().await {
            InputEvent::Input(key) => app.do_action(key).await,
            InputEvent::Tick => app.update_on_tick().await,
        };

        // Check if we should exit
        if result == AppReturn::Exit {
            events.close();
            break;
        }
    }

    terminal.clear()?;
    terminal.show_cursor()?;
    crossterm::terminal::disable_raw_mode()?;

    println!("");
    Ok(())
}
