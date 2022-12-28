use tui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Spans, Text},
    widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Table, TableState},
    Frame,
};

use super::App;

pub fn draw<B>(rect: &mut Frame<B>, app: &App)
where
    B: Backend,
{
    let size = rect.size();
    // TODO check size

    // Vertical layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(size.height - 2), Constraint::Length(2)].as_ref())
        .split(size);

    draw_body(rect, chunks, app);
}

fn draw_body<B>(frame: &mut Frame<B>, chunks: Vec<Rect>, app: &App)
where
    B: Backend,
{
    if app.state.is_monitoring() {
        let available_width = chunks[0].width as usize;

        let containers = app.containers();

        let selected_style = Style::default().add_modifier(Modifier::REVERSED);

        let header_cells = ["", "ID", "SERVICE", "CPU%", "MEM", "STACK"]
            .iter()
            .map(|h| Cell::from(*h).style(Style::default().fg(Color::LightCyan)));
        let header = Row::new(header_cells).height(1).bottom_margin(1);
        let rows = containers.iter().map(|c| {
            let status = &c.status;
            let status_label = match status {
                crate::app::container_management::ContainerStatus::Created => {
                    Span::styled(" ", Style::default().bg(Color::Gray))
                }
                crate::app::container_management::ContainerStatus::Running => {
                    Span::styled(" ", Style::default().bg(Color::Green))
                }
                crate::app::container_management::ContainerStatus::Paused => {
                    Span::styled(" ", Style::default().bg(Color::Yellow))
                }
                crate::app::container_management::ContainerStatus::Stopped
                | crate::app::container_management::ContainerStatus::Exited => {
                    Span::styled(" ", Style::default().bg(Color::Red))
                }
                crate::app::container_management::ContainerStatus::Restarting => {
                    Span::styled(" ", Style::default().bg(Color::LightGreen))
                }
                crate::app::container_management::ContainerStatus::Removing => {
                    Span::styled(" ", Style::default().bg(Color::LightRed))
                }
                crate::app::container_management::ContainerStatus::Dead => {
                    Span::styled(" ", Style::default().bg(Color::Black))
                }
            };
            let cpu = c.cpu_usage;
            let mem_usage = c.memory_usage_bytes;
            let mem_total = c.memory_limit_bytes;
            let stack = c
                .swarm_stack
                .clone()
                .unwrap_or(c.compose_project.clone().unwrap_or_default());
            let service = c
                .swarm_service
                .clone()
                .unwrap_or(c.compose_service.clone().unwrap_or_default())
                .replace(format!("{}_", stack).as_str(), "");

            let mem = label_for_memory(mem_usage, mem_total);
            let mem_width: usize = (available_width as f32 * 0.2) as usize;
            let num_green_chars = (mem_usage / mem_total * mem_width as f32) as usize;
            let mut mem_label = vec![' ' as u8; mem_width];
            // let start = mem_width - mem.chars().count() / 2;
            for (i, c) in mem.chars().enumerate() {
                if i >= 30 {
                    break;
                }
                mem_label[i] = c as u8;
            }
            let green_label = String::from_utf8(mem_label[0..num_green_chars].to_vec()).unwrap();
            let normal_label = String::from_utf8(mem_label[num_green_chars..].to_vec()).unwrap();
            let mem_label = Spans::from(vec![
                Span::styled(green_label, Style::default().bg(Color::Green)),
                Span::styled(normal_label, Style::default().bg(Color::DarkGray)),
            ]);

            Row::new(vec![
                Cell::from(status_label),
                Cell::from(c.id.clone()),
                // Cell::from(c.name.clone()),
                Cell::from(service),
                Cell::from(label_for_cpu(cpu)),
                Cell::from(mem_label),
                Cell::from(stack),
            ])
            .height(1)
            .bottom_margin(0)
        });

        let t = Table::new(rows)
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::TOP)
                    .title("Container Monitoring"),
            )
            .highlight_style(selected_style)
            .widths(&[
                Constraint::Length(1),  // Status
                Constraint::Length(12), // ID
                // Constraint::Percentage(15), // Name
                Constraint::Percentage(15), // SERVICE
                Constraint::Length(5),      // CPU
                Constraint::Percentage(20), // MEM
                Constraint::Percentage(15), // STACK
            ])
            .column_spacing(2);

        let mut table_state = TableState::default();
        table_state.select(app.selected_container_index());

        frame.render_stateful_widget(t, chunks[0], &mut table_state);

        draw_help(frame, chunks[1], format!("{}", app.actions()).as_str());
    } else if app.state().is_logging() {
        let logs = app.logs();
        let available_height = chunks[0].height as usize - 1; // -1 for the TOP border
        let available_width = chunks[0].width as usize;
        let pos = app.log_position();

        let logs_iter = logs.iter().rev().take(available_height + pos).rev();
        let mut logs = Text::raw("");
        for l in logs_iter {
            let mut i = available_width;
            let mut line = String::new();
            loop {
                line.extend(l.chars().skip(i - available_width).take(available_width));
                if i > l.chars().count() {
                    break;
                }
                i += available_width;
                line.push('\n');
            }

            let t = if let Some(s) = app.search() {
                if line.contains(s) {
                    let mut content = vec![];
                    if line.starts_with(s) {
                        content.push(Span::styled(s, Style::default().fg(Color::Yellow)));
                    }
                    let lv: Vec<String> = line.split(s).map(|e| e.to_owned()).collect();
                    for segment in lv.iter() {
                        content.push(Span::raw(segment.to_owned()));
                        if lv.last() != Some(&segment) {
                            content.push(Span::styled(s, Style::default().fg(Color::Yellow)));
                        }
                    }
                    if line.ends_with(s) {
                        content.push(Span::styled(s, Style::default().fg(Color::Yellow)));
                    }
                    let mut txt = Text::raw("");
                    txt.lines = vec![Spans::from(content)];
                    txt
                } else {
                    Text::raw(line)
                }
            } else {
                Text::raw(line)
            };
            logs.extend(t);
        }

        let p = Paragraph::new(logs).block(Block::default().borders(Borders::TOP).title(format!(
            "Logs for {}",
            app.selected_container().as_ref().unwrap()
        )));
        frame.render_widget(p, chunks[0]);
        if app.search().is_some() {
            draw_search(frame, app.search().as_ref().unwrap());
        } else {
            draw_help(frame, chunks[1], format!("{}", app.actions()).as_str());
        }
    } else if app.state().is_exec_command() {
        let logs = app.logs();
        let available_height = chunks[0].height as usize - 1; // -1 for the TOP border
        let available_width = chunks[0].width as usize;
        let mut logs = match logs
            .iter()
            .rev()
            .take(available_height / 2)
            .rev()
            .map(|l| {
                let mut i = available_width;
                let mut line = String::new();
                loop {
                    line.extend(l.chars().skip(i - available_width).take(available_width));
                    if i > l.chars().count() {
                        break;
                    }
                    i += available_width;
                    line.push('\n');
                }
                Text::raw(line)
            })
            .reduce(|mut acc, v| {
                acc.extend(v);
                acc
            }) {
            Some(l) => l,
            None => Text::raw(""),
        }; // TODO show last lines (line breaks hide them)
        logs.extend(Text::raw(app.exec_cmd()));
        let p =
            Paragraph::new(logs).block(Block::default().borders(Borders::TOP).title("Exec CMD"));
        frame.render_widget(p, chunks[0]);
        draw_help(frame, chunks[1], format!("{}", app.actions()).as_str());
    } else {
        let initialized_text = "Not Initialized !";

        let p = Paragraph::new(vec![Spans::from(Span::raw(initialized_text))])
            .style(Style::default().fg(Color::LightCyan))
            .alignment(Alignment::Left)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::White))
                    .border_type(BorderType::Plain),
            );
        frame.render_widget(p, chunks[0]);
    }
}

fn draw_help<B>(frame: &mut Frame<B>, chunk: Rect, help_txt: &str)
where
    B: Backend,
{
    let p = Paragraph::new(vec![Spans::from(Span::raw(help_txt))])
        .style(Style::default().fg(Color::LightCyan))
        .alignment(Alignment::Left)
        .block(
            Block::default()
                .borders(Borders::TOP)
                .style(Style::default().fg(Color::White))
                .title("Help")
                .border_type(BorderType::Plain),
        );
    frame.render_widget(p, chunk);
}

fn draw_search<B>(frame: &mut Frame<B>, search: &str)
where
    B: Backend,
{
    let p = Paragraph::new(vec![Spans::from(Span::raw(search))])
        .style(Style::default().fg(Color::LightCyan))
        .alignment(Alignment::Left)
        .block(
            Block::default()
                .borders(Borders::TOP)
                .title("Search")
                .style(Style::default().fg(Color::White).bg(Color::Black))
                .border_type(BorderType::Plain),
        );
    frame.render_widget(
        p,
        Rect::new(0, frame.size().height - 2, frame.size().width, 2),
    );
}

fn label_for_memory(mem_usage: f32, mem_total: f32) -> String {
    let mem_usage = mem_usage / 1024.0 / 1024.0 / 1024.0;
    let mem_total = mem_total / 1024.0 / 1024.0 / 1024.0;
    format!("{:.2} / {:.2} GB", mem_usage, mem_total)
}

fn label_for_cpu(cpu_usage: f32) -> String {
    format!("{:.2}%", cpu_usage)
}
