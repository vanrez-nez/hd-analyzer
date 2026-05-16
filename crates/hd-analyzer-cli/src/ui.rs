use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::prelude::{Alignment, Color, Line, Modifier, Span, Style};
use ratatui::text::Text;
use ratatui::widgets::{
    Block, Borders, Cell, Clear, List, ListItem, ListState, Paragraph, Row, Table, TableState, Wrap,
};

use crate::app::{
    App, CategoryUsage, FileKind, ScanResult, Screen, compact_path, format_bytes, format_ratio,
};

// Rich Colors (Superfile / Catppuccin Mocha)
const TEXT: Color = Color::Rgb(205, 214, 244);
const SUBTEXT: Color = Color::Rgb(166, 173, 200);
const OVERLAY: Color = Color::Rgb(88, 91, 112);
const GREEN: Color = Color::Rgb(166, 227, 161);
const YELLOW: Color = Color::Rgb(249, 226, 175);
const BLUE: Color = Color::Rgb(137, 180, 250);
const MAGENTA: Color = Color::Rgb(245, 194, 231);
const RED: Color = Color::Rgb(243, 139, 168);
const TEAL: Color = Color::Rgb(148, 226, 213);
const ROSEWATER: Color = Color::Rgb(245, 224, 220);
const MANTLE: Color = Color::Rgb(49, 50, 68);
const CRUST: Color = Color::Rgb(17, 17, 27);

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(2),
        ])
        .split(area);

    draw_header(frame, sections[0], app);
    match app.screen {
        Screen::SelectDrive => draw_drive_selection(frame, sections[1], app),
        Screen::Results => draw_results(frame, sections[1], app),
        Screen::ErrorLog => draw_error_log(frame, sections[1], app),
    }
    draw_footer(frame, sections[2], app);

    if let Some(message) = &app.error_message {
        draw_error(frame, area, message);
    }
}

fn draw_header(frame: &mut Frame, area: Rect, app: &App) {
    let title = match app.screen {
        Screen::SelectDrive => " HD Analyzer",
        Screen::Results => " HD Analyzer // Explorer",
        Screen::ErrorLog => " HD Analyzer // Access Denied Errors",
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(MAGENTA));

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(inner_area);

    let title_para = Paragraph::new(Span::styled(
        title,
        Style::default().fg(MAGENTA).add_modifier(Modifier::BOLD),
    ));
    frame.render_widget(title_para, chunks[0]);

    if chunks.len() > 1 && app.screen == Screen::Results || app.screen == Screen::ErrorLog {
        let right_text = if let Some(result) = &app.result {
            let total = result.total_bytes();
            let drive = app.drives.iter().find(|d| d.mount_point == result.root);
            let drive_usage = drive.map(|d| d.used_space()).unwrap_or(total).max(total);
            let status = if result.completed_at.is_some() {
                "Complete"
            } else {
                "Scanning"
            };

            format!(
                "{} (Elapsed: {}) | {} / {} ",
                status,
                format_elapsed(result.elapsed()),
                format_bytes(total),
                format_bytes(drive_usage)
            )
        } else {
            "Preparing scan engine...".to_string()
        };

        let summary_para = Paragraph::new(Span::styled(
            right_text,
            Style::default().fg(YELLOW).add_modifier(Modifier::BOLD),
        ))
        .alignment(Alignment::Right);
        frame.render_widget(summary_para, chunks[1]);
    }
}

fn draw_drive_selection(frame: &mut Frame, area: Rect, app: &App) {
    let items: Vec<_> = app
        .drives
        .iter()
        .map(|drive| {
            let used = drive.used_space();
            ListItem::new(Text::from(vec![
                Line::from(vec![
                    Span::styled(
                        drive.label.as_str(),
                        Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("  "),
                    Span::styled(drive.file_system.as_str(), Style::default().fg(TEAL)),
                ]),
                Line::from(vec![
                    Span::styled(
                        format!("Used {}", format_bytes(used)),
                        Style::default().fg(YELLOW),
                    ),
                    Span::raw("  "),
                    Span::styled(
                        format!("Free {}", format_bytes(drive.available_space)),
                        Style::default().fg(GREEN),
                    ),
                    Span::raw("  "),
                    Span::styled(
                        format!("Total {}", format_bytes(drive.total_space)),
                        Style::default().fg(SUBTEXT),
                    ),
                ]),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Volumes")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(BLUE)),
        )
        .highlight_style(
            Style::default()
                .bg(OVERLAY)
                .fg(TEXT)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    let mut state = ListState::default().with_selected(Some(app.selected_drive));
    frame.render_stateful_widget(list, area, &mut state);
}

fn draw_results(frame: &mut Frame, area: Rect, app: &App) {
    let Some(result) = &app.result else {
        return;
    };

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(62), Constraint::Percentage(38)])
        .split(area);

    draw_largest_directories(frame, body[0], app, result);
    draw_categories(frame, body[1], result);
}

fn draw_largest_directories(frame: &mut Frame, area: Rect, app: &App, result: &ScanResult) {
    let total = result.total_bytes();
    let entries = app.current_directory_entries();

    // Breadcrumb style title
    let mut title_spans = vec![Span::raw(" Explorer: ")];
    if let Some(path) = app.current_result_path() {
        let compacted = compact_path(path, &result.root);
        let parts: Vec<&str> = compacted.split(std::path::MAIN_SEPARATOR).collect();
        for (i, part) in parts.iter().enumerate() {
            if i > 0 {
                title_spans.push(Span::styled(" > ", Style::default().fg(OVERLAY)));
            }
            title_spans.push(Span::styled(part.to_string(), Style::default().fg(MAGENTA)));
        }
    } else {
        title_spans.push(Span::styled("Root", Style::default().fg(MAGENTA)));
    }
    title_spans.push(Span::raw(" "));
    let title = Line::from(title_spans);

    let mut rows: Vec<_> = entries
        .iter()
        .map(|entry| {
            let name = entry
                .path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("..");
            let is_root = entry.path == result.root;
            let display_name = if is_root { ".." } else { name };
            Row::new(vec![
                Cell::from(Span::styled(
                    format!("📁 {}", display_name),
                    Style::default().fg(TEXT),
                )),
                Cell::from(Span::styled(
                    format_bytes(entry.size),
                    Style::default().fg(YELLOW),
                )),
                Cell::from(Span::styled(
                    format_ratio(entry.size, total),
                    Style::default().fg(SUBTEXT),
                )),
            ])
        })
        .collect();

    let is_root = app.result_path.as_ref() == Some(&result.root) || app.result_path.is_none();
    if is_root {
        if let Some(drive) = app.drives.iter().find(|d| d.mount_point == result.root) {
            let drive_used = drive.used_space();
            if drive_used > total + 1_000_000 {
                let hidden = drive_used - total;
                rows.push(Row::new(vec![
                    Cell::from(Span::styled(
                        "🔒  Hidden / Unscanned",
                        Style::default().fg(OVERLAY),
                    )),
                    Cell::from(Span::styled(
                        format_bytes(hidden),
                        Style::default().fg(OVERLAY),
                    )),
                    Cell::from(Span::styled("---", Style::default().fg(OVERLAY))),
                ]));
            }
        }
    }

    let table = Table::new(
        rows,
        [
            Constraint::Min(30),
            Constraint::Length(12),
            Constraint::Length(8),
        ],
    )
    .header(
        Row::new(vec!["Directory", "Size", "Share"])
            .style(Style::default().fg(BLUE).add_modifier(Modifier::BOLD))
            .bottom_margin(1),
    )
    .block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(BLUE)),
    )
    .row_highlight_style(Style::default().bg(MANTLE).add_modifier(Modifier::BOLD))
    .column_spacing(1);

    let mut state = TableState::default()
        .with_selected((!entries.is_empty()).then_some(app.selected_result_entry));
    frame.render_stateful_widget(table, area, &mut state);
}

fn draw_categories(frame: &mut Frame, area: Rect, result: &ScanResult) {
    if result.categories.is_empty() {
        let block = Block::default()
            .title(" Space By File Type")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(OVERLAY));
        frame.render_widget(Clear, area);
        frame.render_widget(block, area);
        return;
    }

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(8)])
        .split(area);

    let bar = Paragraph::new(stacked_bar_line(
        &result.categories,
        result.total_bytes(),
        sections[0].width,
    ))
    .block(
        Block::default()
            .title(" Usage Distribution")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(OVERLAY)),
    )
    .alignment(Alignment::Left);
    frame.render_widget(bar, sections[0]);

    let rows: Vec<_> = result
        .categories
        .iter()
        .filter(|entry| entry.size > 0)
        .map(|entry| {
            Row::new(vec![
                Cell::from(Span::styled(
                    entry.kind.label(),
                    Style::default().fg(color_for_kind(entry.kind)),
                )),
                Cell::from(format_bytes(entry.size)),
                Cell::from(format_ratio(entry.size, result.total_bytes())),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Length(8),
        ],
    )
    .header(
        Row::new(vec!["Type", "Size", "Share"])
            .style(Style::default().fg(SUBTEXT).add_modifier(Modifier::BOLD)),
    )
    .column_spacing(1);

    let legend_block = Block::default()
        .title(" Legend")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(OVERLAY));
    let legend_inner = legend_block.inner(sections[1]);
    frame.render_widget(legend_block, sections[1]);

    let inner_sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(legend_inner);

    frame.render_widget(table, inner_sections[0]);

    let summary = Line::from(vec![
        Span::styled(format!(" Total: "), Style::default().fg(SUBTEXT)),
        Span::styled(
            format!(
                "{} files, {} dirs",
                result.files_scanned, result.directories_scanned
            ),
            Style::default().fg(TEXT),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(summary).alignment(Alignment::Left),
        inner_sections[1],
    );
}

fn draw_error_log(frame: &mut Frame, area: Rect, app: &App) {
    let Some(result) = &app.result else {
        return;
    };

    let items: Vec<ListItem> = result
        .read_errors
        .iter()
        .map(|err| {
            let path = compact_path(&err.path, &result.root);
            let content = vec![
                Line::from(vec![
                    Span::styled("🔒 ", Style::default().fg(RED)),
                    Span::styled(path, Style::default().fg(TEXT).add_modifier(Modifier::BOLD)),
                ]),
                Line::from(vec![
                    Span::raw("   "),
                    Span::styled(&err.error, Style::default().fg(SUBTEXT)),
                ]),
                Line::from(vec![Span::raw("")]),
            ];
            ListItem::new(content)
        })
        .collect();

    let list = ratatui::widgets::List::new(items)
        .block(
            Block::default()
                .title(" Permission Denied ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(RED)),
        )
        .highlight_style(Style::default().bg(MANTLE));

    let mut state = ratatui::widgets::ListState::default().with_selected(Some(app.error_scroll));
    frame.render_stateful_widget(list, area, &mut state);
}

fn draw_footer(frame: &mut Frame, area: Rect, app: &App) {
    let help = match app.screen {
        Screen::SelectDrive => "↑/↓/j/k: Select   Enter: Scan   q: Quit",
        Screen::Results => {
            "↑/↓/j/k: Select   Enter/l: Open   Backspace/h: Parent   r: Rescan   s: Drives   q: Quit"
        }
        Screen::ErrorLog => "↑/↓/j/k: Scroll   Esc/Backspace/h: Go Back   q: Quit",
    };

    let paragraph = Paragraph::new(help)
        .style(Style::default().fg(SUBTEXT).bg(CRUST))
        .alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

fn draw_error(frame: &mut Frame, area: Rect, message: &str) {
    let popup = centered_rect(60, 20, area);
    let text = Paragraph::new(message)
        .block(
            Block::default()
                .title("Error")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(RED)),
        )
        .style(Style::default().fg(TEXT).bg(CRUST))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    frame.render_widget(Clear, popup);
    frame.render_widget(text, popup);
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

fn stacked_bar_line(categories: &[CategoryUsage], total: u64, width: u16) -> Line<'static> {
    let inner_width = width.saturating_sub(2) as usize;
    if total == 0 || inner_width == 0 {
        return Line::from(vec![Span::raw("No data yet")]);
    }

    let mut remaining = inner_width;
    let mut spans = Vec::new();
    let non_zero: Vec<_> = categories.iter().filter(|entry| entry.size > 0).collect();
    if non_zero.is_empty() {
        return Line::from(vec![Span::raw("No data yet")]);
    }

    for (index, entry) in non_zero.iter().enumerate() {
        let is_last = index + 1 == non_zero.len();
        let segment = if is_last {
            remaining
        } else {
            ((entry.size as f64 / total as f64) * inner_width as f64).round() as usize
        }
        .min(remaining);

        if segment > 0 {
            spans.push(Span::styled(
                "█".repeat(segment),
                Style::default().fg(color_for_kind(entry.kind)),
            ));
            remaining = remaining.saturating_sub(segment);
        }
    }

    if remaining > 0 {
        spans.push(Span::styled(
            "█".repeat(remaining),
            Style::default().fg(OVERLAY),
        ));
    }

    Line::from(spans)
}

fn color_for_kind(kind: FileKind) -> Color {
    match kind {
        FileKind::Video => RED,
        FileKind::Audio => BLUE,
        FileKind::Images => MAGENTA,
        FileKind::Archives => ROSEWATER,
        FileKind::Apps => YELLOW,
        FileKind::Documents => TEAL,
        FileKind::Code => GREEN,
        FileKind::Other => SUBTEXT,
    }
}

fn format_elapsed(duration: std::time::Duration) -> String {
    let total_seconds = duration.as_secs();
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    if hours > 0 {
        format!("{hours}h {minutes}m {seconds}s")
    } else if minutes > 0 {
        format!("{minutes}m {seconds}s")
    } else {
        format!("{seconds}s")
    }
}
