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
        let containers = app.containers();

        let selected_style = Style::default().add_modifier(Modifier::REVERSED);

        let header_cells = ["", "ID", "Name", "CPU%", "MEM", "SERVICE", "STACK"]
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
                crate::app::container_management::ContainerStatus::Stopped => {
                    Span::styled(" ", Style::default().bg(Color::Red))
                }
                crate::app::container_management::ContainerStatus::Restarting => {
                    Span::styled(" ", Style::default().bg(Color::LightGreen))
                }
                crate::app::container_management::ContainerStatus::Removing => {
                    Span::styled(" ", Style::default().bg(Color::LightRed))
                }
                crate::app::container_management::ContainerStatus::Exited => {
                    Span::styled(" ", Style::default().bg(Color::LightMagenta))
                }
                crate::app::container_management::ContainerStatus::Dead => {
                    Span::styled(" ", Style::default().bg(Color::Black))
                }
            };
            let cpu = c.cpu_usage;
            let mem_usage = c.memory_usage_bytes;
            let mem_total = c.memory_limit_bytes;
            let service = c.swarm_service.clone().unwrap_or_default();
            let stack = c.swarm_stack.clone().unwrap_or_default();

            let mem = label_for_memory(mem_usage, mem_total);
            const MEM_WIDTH: usize = 30;
            let num_green_chars = (mem_usage / mem_total * MEM_WIDTH as f32) as usize;
            let mut mem_label = [' ' as u8; MEM_WIDTH];
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
                Span::raw(normal_label),
            ]);

            Row::new(vec![
                Cell::from(status_label),
                Cell::from(c.id.clone()),
                Cell::from(c.name.clone()),
                Cell::from(label_for_cpu(cpu)),
                Cell::from(mem_label),
                Cell::from(service),
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
                Constraint::Length(30), // Name
                Constraint::Length(5),  // CPU
                Constraint::Length(30), // MEM
                Constraint::Length(25), // SERVICE
                Constraint::Length(15), // STACK
            ])
            .column_spacing(1);

        let mut table_state = TableState::default();
        table_state.select(app.selected_container_index());

        frame.render_stateful_widget(t, chunks[0], &mut table_state);
        draw_help(frame, chunks[1], "q: Quit | l: Show Logs");
    } else if app.state().is_logging() {
        let logs = app.logs();
        let available_height = chunks[0].height as usize - 1; // -1 for the TOP border
        let available_width = chunks[0].width as usize;
        let pos = app.log_position();
        let logs = logs
            .iter()
            .rev()
            .take(available_height + pos)
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
            })
            .unwrap(); // TODO show last lines (line breaks hide them)

        let p = Paragraph::new(logs).block(Block::default().borders(Borders::TOP).title("Logs"));
        frame.render_widget(p, chunks[0]);
        draw_help(frame, chunks[1], "q: Quit");
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

fn label_for_memory(mem_usage: f32, mem_total: f32) -> String {
    let mem_usage = mem_usage / 1024.0 / 1024.0 / 1024.0;
    let mem_total = mem_total / 1024.0 / 1024.0 / 1024.0;
    format!("{:.2} / {:.2} GB", mem_usage, mem_total)
}

fn label_for_cpu(cpu_usage: f32) -> String {
    format!("{:.2}%", cpu_usage)
}
