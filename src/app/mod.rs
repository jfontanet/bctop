pub mod actions;
pub mod container_management;
pub mod state;
pub mod ui;

use crate::{inputs::key::Key, io::IoEvent};
use actions::{Action, Actions};
use chrono::TimeZone;
// use log::{debug, error, warn};
use state::AppState;

use self::container_management::{Container, ContainerManagement};

#[derive(Debug, PartialEq, Eq)]
pub enum AppReturn {
    Exit,
    Continue,
}

pub struct App {
    containers: Vec<Container>,
    /// We could dispatch an IO event
    io_tx: tokio::sync::mpsc::Sender<IoEvent>,
    /// Contextual actions
    actions: Actions,
    state: AppState,
    selected_container: Option<String>,
    // Logging attributes
    logs: Vec<String>,
    last_log_ts: chrono::DateTime<chrono::Utc>,
    log_position: usize, // Reverse index from where to start taking log lines
    search: Option<String>,
    // Execution attributes
    // exec_tx: tokio::sync::mpsc::Sender<String>,
    // exec_rx: tokio::sync::mpsc::Receiver<String>,
    exec_cmd: String,
}

impl App {
    pub fn new(io_tx: tokio::sync::mpsc::Sender<IoEvent>) -> Self {
        let state = AppState::default();
        let actions = state.get_actions();
        let containers = Vec::new();

        Self {
            containers,
            io_tx,
            actions,
            state,
            selected_container: None,
            logs: Vec::new(),
            last_log_ts: chrono::Utc.timestamp(0, 0),
            log_position: 0,
            search: None,
            exec_cmd: String::new(),
        }
    }

    /// Handle a user action
    pub async fn do_action(&mut self, key: Key) -> AppReturn {
        if self.search().is_some() {
            if let Some(c) = key.get_char() {
                self.search = Some(format!("{}{}", self.search().as_ref().unwrap(), c));
                return AppReturn::Continue;
            }
        }
        if self.state().is_exec_command() {
            if let Some(c) = key.get_char() {
                self.exec_cmd.push(c);
                return AppReturn::Continue;
            }
        }
        if let Some(action) = self.actions.find(key) {
            if self.state.is_monitoring() {
                self.do_state_monitoring_actions(*action).await
            } else if self.state.is_logging() {
                self.do_state_logging_actions(*action).await
            } else if self.state.is_exec_command() {
                self.do_state_exec_command_actions(*action).await
            } else {
                AppReturn::Continue
            }
        } else {
            AppReturn::Continue
        }
    }

    async fn do_state_monitoring_actions(&mut self, action: Action) -> AppReturn {
        match action {
            Action::Quit => AppReturn::Exit,
            Action::ShowLogs => {
                if self.selected_container.is_none() {
                    return AppReturn::Continue; // No container selected, do nothing
                }
                self.state = AppState::Logging {
                    container: self.selected_container.clone().unwrap(),
                };
                let logs = container_management::get_logs_from(
                    &self.last_log_ts,
                    self.selected_container.clone().unwrap(),
                )
                .await;
                self.last_log_ts = chrono::Utc::now();
                self.logs = logs;
                self.actions = self.state.get_actions();
                AppReturn::Continue
            }
            Action::ExecCommands => {
                if self.selected_container.is_none() {
                    return AppReturn::Continue; // No container selected, do nothing
                }
                self.state = AppState::ExecCommand {
                    container: self.selected_container.clone().unwrap(),
                };
                self.exec_cmd = String::new();
                AppReturn::Continue
            } // TODO
            Action::Next => {
                self.next();
                AppReturn::Continue
            }
            Action::Previous => {
                self.previous();
                AppReturn::Continue
            }
            _ => AppReturn::Continue,
        }
    }

    async fn do_state_logging_actions(&mut self, action: Action) -> AppReturn {
        match action {
            Action::Quit => {
                if self.search.is_some() {
                    self.search = None;
                    return AppReturn::Continue;
                }
                self.state = AppState::Monitoring;
                self.logs.clear();
                self.log_position = 0;
                self.last_log_ts = chrono::Utc.timestamp(0, 0);
                self.actions = self.state.get_actions();
                AppReturn::Continue
            }
            Action::ScrollDown => {
                self.log_position = if self.log_position > 0 {
                    self.log_position - 1
                } else {
                    0
                };
                AppReturn::Continue
            }
            Action::ScrollUp => {
                self.log_position = if self.log_position + 1 < self.logs.len() {
                    self.log_position + 1
                } else {
                    self.log_position
                };
                AppReturn::Continue
            }
            Action::Search => {
                if let Some(search_text) = self.search() {
                    if let Some(line) = self
                        .logs()
                        .iter()
                        .rev()
                        .skip(self.log_position + 1)
                        .position(|line| line.contains(search_text))
                    {
                        self.log_position += line + 1;
                    }
                } else {
                    self.search = Some("".to_string());
                }
                AppReturn::Continue
            }
            Action::Remove => {
                if let Some(search_text) = self.search() {
                    let mut new_text = search_text.clone();
                    new_text.pop();
                    self.search = Some(new_text);
                }
                AppReturn::Continue
            }
            _ => AppReturn::Continue,
        }
    }

    async fn do_state_exec_command_actions(&mut self, action: Action) -> AppReturn {
        match action {
            Action::Quit => {
                self.state = AppState::Monitoring;
                self.actions = self.state.get_actions();
                AppReturn::Continue
            }
            Action::SendCMD => {
                AppReturn::Continue // TODO
            }
            _ => AppReturn::Continue,
        }
    }

    /// We could update the app or dispatch event on tick
    pub async fn update_on_tick(&mut self) -> AppReturn {
        if self.state().is_logging()
            && self.selected_container.is_some()
            && self.log_position() == 0
        {
            let log_lines = container_management::get_logs_from(
                &self.last_log_ts,
                self.selected_container.clone().unwrap(),
            )
            .await;

            self.logs.extend(log_lines);
            self.last_log_ts = chrono::Utc::now();
        }
        AppReturn::Continue
    }

    /// Send a network event to the IO thread
    pub async fn dispatch(&mut self, action: IoEvent) {
        // `is_loading` will be set to false again after the async action has finished in io/handler.rs
        if let Err(_e) = self.io_tx.send(action).await {
            // error!("Error from dispatch {}", e);
        };
    }

    pub fn actions(&self) -> &Actions {
        &self.actions
    }
    pub fn state(&self) -> &AppState {
        &self.state
    }
    pub fn containers(&self) -> &Vec<Container> {
        &self.containers
    }
    pub fn selected_container(&self) -> &Option<String> {
        &self.selected_container
    }
    pub fn selected_container_index(&self) -> Option<usize> {
        self.selected_container
            .as_ref()
            .and_then(|id| self.containers.iter().position(|c| c.id == *id))
    }
    pub fn logs(&self) -> &Vec<String> {
        &self.logs
    }
    pub fn log_position(&self) -> usize {
        self.log_position
    }
    pub fn search(&self) -> &Option<String> {
        &self.search
    }
    pub fn exec_cmd(&self) -> &String {
        &self.exec_cmd
    }

    pub fn next(&mut self) {
        let index = match &self.selected_container {
            Some(i) => {
                let idx = self.containers.iter().position(|c| c.id == *i).unwrap_or(0);
                if idx + 1 >= self.containers.len() {
                    idx
                } else {
                    idx + 1
                }
            }
            None => 0,
        };

        self.selected_container = self
            .containers
            .get(index)
            .map_or(None, |c| Some(c.id.clone()));
    }

    pub fn previous(&mut self) {
        let index = match &self.selected_container {
            Some(i) => {
                let idx = self.containers.iter().position(|c| c.id == *i).unwrap_or(0);
                if idx == 0 {
                    idx
                } else {
                    idx - 1
                }
            }
            None => 0,
        };

        self.selected_container = self
            .containers
            .get(index as usize)
            .map_or(None, |c| Some(c.id.clone()));
    }
}

impl ContainerManagement for App {
    fn update_containers(&mut self, new_container: Container) {
        self.containers.retain(|c| c.id != new_container.id);
        self.containers.push(new_container);
        self.containers.sort_by(|a, b| a.name.cmp(&b.name));
    }

    fn remove_container(&mut self, id: &str) {
        self.containers.retain(|c| c.id != id);
    }
}
