pub mod actions;
use crate::container_management;
pub mod state;
pub mod ui;

use crate::io::SessionObject;
use crate::{inputs::key::Key, io::IoEvent};
use actions::{Action, Actions};
use log::debug;
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
    log_position: usize, // Reverse index from where to start taking log lines
    search: Option<String>,
    // Execution attributes
    exec_tx: Option<tokio::sync::mpsc::Sender<String>>,
    exec_cmd: String,
    last_cmd: Option<String>,
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
            log_position: 0,
            search: None,
            exec_tx: None,
            exec_cmd: String::new(),
            last_cmd: None,
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
                self.actions = self.state.get_actions();
                self.dispatch(IoEvent::ShowLogs(self.selected_container.clone().unwrap()))
                    .await;
                AppReturn::Continue
            }
            Action::ExecCommands => {
                if self.selected_container.is_none() {
                    return AppReturn::Continue; // No container selected, do nothing
                }
                self.state = AppState::ExecCommand {
                    container: self.selected_container.clone().unwrap(),
                };
                self.actions = self.state.get_actions();
                self.exec_cmd = String::new();

                let (app_tx, exec_rx) = tokio::sync::mpsc::channel::<String>(100);

                self.exec_tx = Some(app_tx);
                self.dispatch(IoEvent::StartExecSession(SessionObject {
                    container_id: self.selected_container.clone().unwrap(),
                    rx_channel: exec_rx,
                }))
                .await;
                AppReturn::Continue
            }
            Action::Next => {
                self.next();
                AppReturn::Continue
            }
            Action::Previous => {
                self.previous();
                AppReturn::Continue
            }
            Action::StopContainer => {
                if self.selected_container.is_none() {
                    return AppReturn::Continue; // No container selected, do nothing
                }
                self.dispatch(IoEvent::StopContainer(
                    self.selected_container.clone().unwrap(),
                ))
                .await;
                AppReturn::Continue
            }
            Action::PauseContainer => {
                if self.selected_container.is_none() {
                    return AppReturn::Continue; // No container selected, do nothing
                }
                self.dispatch(IoEvent::PauseContainer(
                    self.selected_container.clone().unwrap(),
                ))
                .await;
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
                self.actions = self.state.get_actions();
                self.dispatch(IoEvent::StartMonitoring).await;
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
                        .position(|line| {
                            line.to_lowercase()
                                .contains(&search_text.clone().to_lowercase())
                        })
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
            // TODO: Handle exists
            Action::Quit => {
                self.state = AppState::Monitoring;
                self.actions = self.state.get_actions();
                self.logs.clear();
                self.log_position = 0;
                if let Some(tx_ch) = self.exec_tx.as_ref() {
                    tx_ch.send(format!("exit\n")).await.unwrap();
                }
                AppReturn::Continue
            }
            Action::SendCMD => {
                if let Some(tx_ch) = self.exec_tx.as_ref() {
                    self.exec_cmd.push_str("\n");
                    if let Some(last) = self.logs.last_mut() {
                        *last = format!("{}{}", last, self.exec_cmd);
                    }
                    tx_ch.send(self.exec_cmd.clone()).await.unwrap();
                    self.last_cmd = Some(self.exec_cmd.clone());
                    self.exec_cmd = String::new();
                }
                AppReturn::Continue
            }
            _ => AppReturn::Continue,
        }
    }

    /// We could update the app or dispatch event on tick
    pub async fn update_on_tick(&mut self) -> AppReturn {
        AppReturn::Continue
    }

    /// Send a network event to the IO thread
    pub async fn dispatch(&mut self, action: IoEvent) {
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

    fn add_logs(&mut self, logs: Vec<String>) {
        if self.log_position != 0 {
            self.log_position += logs.len();
        }
        self.logs.extend(logs);
    }

    fn add_tty_output(&mut self, output: String) {
        debug!("TTY Output: {}", output);
        if output == "exit" {
            self.state = AppState::Monitoring;
            self.actions = self.state.get_actions();
            self.logs.clear();
            self.log_position = 0;
        } else if self.state.is_exec_command() {
            if let Some(cmd) = &self.last_cmd {
                if output.trim() == cmd.to_owned().trim() {
                    self.last_cmd = None;
                    return;
                }
            }
            self.logs.push(output);
        }
    }
}
