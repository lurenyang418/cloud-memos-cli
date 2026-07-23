use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Clear, List, ListItem, ListState, Paragraph, Tabs, Wrap},
};

use crate::{
    app::{App, UiMode, View},
    model::{Memo, MemoVersion},
    security::sanitize_terminal,
};

const MIN_WIDTH: u16 = 40;
const MIN_HEIGHT: u16 = 10;
const WIDE_WIDTH: u16 = 100;

pub fn render(frame: &mut Frame<'_>, app: &mut App) {
    let area = frame.area();
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        frame.render_widget(
            Paragraph::new(format!(
                "终端窗口太小\n当前 {}×{}，至少需要 {MIN_WIDTH}×{MIN_HEIGHT}",
                area.width, area.height
            ))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true })
            .block(Block::bordered().title("Cloud Memos")),
            area,
        );
        return;
    }

    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(4),
            Constraint::Length(2),
        ])
        .split(area);
    render_header(frame, app, outer[0]);
    render_body(frame, app, outer[1]);
    render_footer(frame, app, outer[2]);

    match app.mode {
        UiMode::Browse => {}
        UiMode::Edit => render_editor(frame, app, area),
        UiMode::Filter => render_filter(frame, app, area),
        UiMode::Confirm => render_confirm(frame, app, area),
        UiMode::Help => render_help(frame, area),
        UiMode::History => render_history(frame, app, area),
        UiMode::Conflict => render_conflict(frame, app, area),
    }
}

fn render_header(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let titles = View::ALL
        .iter()
        .map(|view| Line::from(format!(" {} ", view.label())))
        .collect::<Vec<_>>();
    let title = format!(
        " {} · {} · {} ",
        app.app_name,
        app.viewer_name,
        app.access_mode.label()
    );
    let tabs = Tabs::new(titles)
        .select(app.view.index())
        .block(Block::bordered().title(title))
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .divider("│");
    frame.render_widget(tabs, area);
}

fn render_body(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    if area.width >= WIDE_WIDTH {
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(43), Constraint::Percentage(57)])
            .split(area);
        render_list(frame, app, columns[0]);
        render_detail(frame, app.selected_memo(), columns[1]);
    } else if app.show_detail {
        render_detail(frame, app.selected_memo(), area);
    } else {
        render_list(frame, app, area);
    }
}

fn render_list(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let title = filter_summary(app);
    if app.items.is_empty() {
        let message = if let Some(error) = &app.last_error {
            format!("加载失败\n{}\n\n按 r 重试", sanitize_terminal(error))
        } else {
            format!("{}中还没有记录", app.view.label())
        };
        frame.render_widget(
            Paragraph::new(message)
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true })
                .block(Block::bordered().title(title)),
            area,
        );
        return;
    }

    let rows = app
        .items
        .iter()
        .map(|memo| {
            let pin = if memo.pinned { "📌 " } else { "" };
            let preview = one_line_preview(&memo.content, 52);
            let first = Line::from(vec![
                Span::styled(pin, Style::default().fg(Color::Yellow)),
                Span::raw(preview),
            ]);
            let second = Line::styled(
                format!(
                    "@{} · {} · v{}{}",
                    sanitize_terminal(&memo.author.username),
                    memo.visibility.label(),
                    memo.version,
                    if memo.attachments.is_empty() {
                        String::new()
                    } else {
                        format!(" · {} 个附件", memo.attachments.len())
                    }
                ),
                Style::default().fg(Color::DarkGray),
            );
            ListItem::new(vec![first, second])
        })
        .collect::<Vec<_>>();
    let mut state = ListState::default().with_selected(Some(app.selected));
    let list = List::new(rows)
        .block(Block::bordered().title(title))
        .highlight_symbol("▶ ")
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(35, 48, 60))
                .add_modifier(Modifier::BOLD),
        );
    frame.render_stateful_widget(list, area, &mut state);
}

fn filter_summary(app: &App) -> String {
    let mut parts = vec![format!(
        " {} · 第 {} 页 ",
        app.view.label(),
        app.page_index + 1
    )];
    if let Some(query) = &app.filters.q {
        parts.push(format!("搜索:{}", sanitize_terminal(query)));
    }
    if let Some(tag) = &app.filters.tag {
        parts.push(format!("标签:{}", sanitize_terminal(tag)));
    }
    if let Some(visibility) = app.filters.visibility {
        parts.push(format!("可见性:{}", visibility.label()));
    }
    parts.join(" · ")
}

fn render_detail(frame: &mut Frame<'_>, memo: Option<&Memo>, area: Rect) {
    let Some(memo) = memo else {
        frame.render_widget(
            Paragraph::new("选择一条记录查看详情")
                .alignment(Alignment::Center)
                .block(Block::bordered().title("详情")),
            area,
        );
        return;
    };

    let mut lines = vec![
        Line::styled(
            format!(
                "{} · {} · v{} · {}",
                sanitize_terminal(&memo.author.name),
                memo.visibility.label(),
                memo.version,
                if memo.pinned {
                    "已置顶"
                } else {
                    "未置顶"
                }
            ),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Line::from(""),
    ];
    lines.extend(markdown_lines(&memo.content));
    if !memo.tags.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::styled(
            format!(
                "标签：{}",
                memo.tags
                    .iter()
                    .map(|tag| format!("#{}", sanitize_terminal(tag)))
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
            Style::default().fg(Color::Magenta),
        ));
    }
    if !memo.attachments.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::styled(
            "附件（仅元数据）",
            Style::default().add_modifier(Modifier::BOLD),
        ));
        for attachment in &memo.attachments {
            lines.push(Line::from(format!(
                "• {} · {} · {}",
                sanitize_terminal(&attachment.filename),
                sanitize_terminal(&attachment.content_type),
                format_bytes(attachment.size)
            )));
        }
    }
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .wrap(Wrap { trim: false })
            .block(Block::bordered().title(" Markdown 详情 ")),
        area,
    );
}

fn render_footer(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let error = app
        .last_error
        .as_ref()
        .map(|message| format!(" · 错误：{}", one_line_preview(message, 60)))
        .unwrap_or_default();
    frame.render_widget(
        Paragraph::new(format!(
            " {}{}  │  ←/→ 视图  j/k 导航  / 搜索  n/e 编辑  r 刷新  ? 帮助  q 退出",
            sanitize_terminal(&app.status),
            error
        ))
        .style(Style::default().fg(if app.last_error.is_some() {
            Color::LightRed
        } else {
            Color::DarkGray
        })),
        area,
    );
}

fn render_editor(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let popup = centered_rect(90, 86, area);
    frame.render_widget(Clear, popup);
    let block = Block::bordered().title(" 编辑记录 · Ctrl+S 保存 · Ctrl+E 外部编辑 · Esc 取消 ");
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    if let Some(editor) = &app.editor {
        frame.render_widget(&editor.textarea, inner);
    }
}

fn render_filter(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let popup = centered_rect(72, 18, area);
    frame.render_widget(Clear, popup);
    let label = app
        .filter
        .as_ref()
        .map_or("筛选", |filter| filter.kind.label());
    let block = Block::bordered().title(format!(" {label} · Enter 应用 · Esc 取消 "));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    if let Some(filter) = &app.filter {
        frame.render_widget(&filter.textarea, inner);
    }
}

fn render_confirm(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let popup = centered_rect(62, 24, area);
    frame.render_widget(Clear, popup);
    let prompt = app
        .confirm
        .as_ref()
        .map_or("确认操作？", |confirm| confirm.prompt.as_str());
    frame.render_widget(
        Paragraph::new(format!("{prompt}\n\n按 y/Enter 确认，按 n/Esc 取消"))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true })
            .block(
                Block::bordered()
                    .title(" 请确认 ")
                    .border_style(Style::default().fg(Color::Yellow)),
            ),
        popup,
    );
}

fn render_help(frame: &mut Frame<'_>, area: Rect) {
    let popup = centered_rect(84, 86, area);
    frame.render_widget(Clear, popup);
    let help = [
        "方向键 / j k    导航记录      ← → / Tab    切换四个视图",
        "PgDown/PgUp     下一页/上一页  Enter         窄屏切换详情",
        "/                搜索          t             标签筛选",
        "f                可见性筛选    r             刷新/错误重试",
        "n / e            新建/编辑     Ctrl+S        保存编辑",
        "Ctrl+E           外部编辑器    v             切换记录可见性",
        "p                置顶          a             归档/取消归档",
        "d                删除          u             从回收站恢复",
        "h                历史版本      ?             帮助",
        "q                退出          Esc           取消/返回",
        "",
        "成员动态只读；附件仅显示元数据。服务器拒绝写 scope 后，本次运行自动降级只读。",
    ]
    .join("\n");
    frame.render_widget(
        Paragraph::new(help)
            .wrap(Wrap { trim: false })
            .block(Block::bordered().title(" 快捷键 · 按 ? / Esc 关闭 ")),
        popup,
    );
}

fn render_history(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let popup = centered_rect(84, 84, area);
    frame.render_widget(Clear, popup);
    if app.history.is_empty() {
        frame.render_widget(
            Paragraph::new("没有可用历史版本\n\nEsc 返回")
                .alignment(Alignment::Center)
                .block(Block::bordered().title(" 历史版本 ")),
            popup,
        );
        return;
    }
    let sections = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(38), Constraint::Percentage(62)])
        .split(popup);
    let rows = app.history.iter().map(history_item).collect::<Vec<_>>();
    let mut state = ListState::default().with_selected(Some(app.history_selected));
    frame.render_stateful_widget(
        List::new(rows)
            .block(Block::bordered().title(" 历史版本 · Enter/r 恢复 · Esc 返回 "))
            .highlight_symbol("▶ ")
            .highlight_style(Style::default().bg(Color::Rgb(35, 48, 60))),
        sections[0],
        &mut state,
    );
    let detail = app
        .history
        .get(app.history_selected)
        .map_or_else(Vec::new, |version| markdown_lines(&version.content));
    frame.render_widget(
        Paragraph::new(Text::from(detail))
            .wrap(Wrap { trim: false })
            .block(Block::bordered().title(" 版本正文 ")),
        sections[1],
    );
}

fn history_item(version: &MemoVersion) -> ListItem<'static> {
    ListItem::new(vec![
        Line::styled(
            format!(
                "v{} · {} · {}",
                version.version,
                version.visibility.label(),
                if version.pinned {
                    "置顶"
                } else {
                    "未置顶"
                }
            ),
            Style::default().fg(Color::Cyan),
        ),
        Line::raw(one_line_preview(&version.content, 72)),
    ])
}

fn render_conflict(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let popup = centered_rect(94, 88, area);
    frame.render_widget(Clear, popup);
    let block = Block::bordered()
        .title(" 版本冲突 · m 合并编辑 · o 确认覆盖 · c/Esc 取消并保留草稿 ")
        .border_style(Style::default().fg(Color::LightRed));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    let Some(conflict) = &app.conflict else {
        return;
    };
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(inner);
    frame.render_widget(
        Paragraph::new(sanitize_terminal(&conflict.local.content))
            .wrap(Wrap { trim: false })
            .block(Block::bordered().title(" 本地草稿（已保留） ")),
        columns[0],
    );
    frame.render_widget(
        Paragraph::new(sanitize_terminal(&conflict.server.content))
            .wrap(Wrap { trim: false })
            .block(Block::bordered().title(format!(" 服务端 v{} ", conflict.server.version))),
        columns[1],
    );
}

fn markdown_lines(input: &str) -> Vec<Line<'static>> {
    let safe = sanitize_terminal(input);
    let parser = Parser::new_ext(
        &safe,
        Options::ENABLE_STRIKETHROUGH
            | Options::ENABLE_TABLES
            | Options::ENABLE_TASKLISTS
            | Options::ENABLE_FOOTNOTES,
    );
    let mut lines = Vec::new();
    let mut current = Vec::new();
    let mut style = Style::default();
    let mut styles = Vec::new();

    for event in parser {
        match event {
            Event::Start(tag) => {
                styles.push(style);
                style = match tag {
                    Tag::Strong => style.add_modifier(Modifier::BOLD),
                    Tag::Emphasis => style.add_modifier(Modifier::ITALIC),
                    Tag::Strikethrough => style.add_modifier(Modifier::CROSSED_OUT),
                    Tag::Heading { .. } => style
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                    Tag::CodeBlock(_) => style.fg(Color::LightGreen),
                    Tag::Link { .. } => style.fg(Color::Blue).add_modifier(Modifier::UNDERLINED),
                    Tag::Item => {
                        current.push(Span::raw("• "));
                        style
                    }
                    _ => style,
                };
            }
            Event::End(end) => {
                style = styles.pop().unwrap_or_default();
                if matches!(
                    end,
                    TagEnd::Paragraph
                        | TagEnd::Heading(_)
                        | TagEnd::Item
                        | TagEnd::CodeBlock
                        | TagEnd::TableRow
                ) {
                    push_line(&mut lines, &mut current);
                }
            }
            Event::Text(text) => append_text(&mut lines, &mut current, &text, style),
            Event::Code(code) => current.push(Span::styled(
                code.into_string(),
                style.fg(Color::LightGreen).add_modifier(Modifier::BOLD),
            )),
            Event::SoftBreak | Event::HardBreak => push_line(&mut lines, &mut current),
            Event::Rule => {
                push_line(&mut lines, &mut current);
                lines.push(Line::styled(
                    "────────────────",
                    Style::default().fg(Color::DarkGray),
                ));
            }
            Event::TaskListMarker(checked) => {
                current.push(Span::raw(if checked { "[x] " } else { "[ ] " }))
            }
            Event::FootnoteReference(reference) => {
                current.push(Span::styled(
                    format!("[{}]", reference.into_string()),
                    Style::default().fg(Color::Blue),
                ));
            }
            Event::Html(html) | Event::InlineHtml(html) => {
                append_text(&mut lines, &mut current, &html, style);
            }
            Event::InlineMath(math) | Event::DisplayMath(math) => {
                current.push(Span::raw(math.into_string()));
            }
        }
    }
    if !current.is_empty() || lines.is_empty() {
        lines.push(Line::from(current));
    }
    lines
}

fn append_text(
    lines: &mut Vec<Line<'static>>,
    current: &mut Vec<Span<'static>>,
    text: &str,
    style: Style,
) {
    let mut segments = text.split('\n').peekable();
    while let Some(segment) = segments.next() {
        if !segment.is_empty() {
            current.push(Span::styled(segment.to_owned(), style));
        }
        if segments.peek().is_some() {
            push_line(lines, current);
        }
    }
}

fn push_line(lines: &mut Vec<Line<'static>>, current: &mut Vec<Span<'static>>) {
    lines.push(Line::from(std::mem::take(current)));
}

fn one_line_preview(input: &str, max_chars: usize) -> String {
    let safe = sanitize_terminal(input);
    let one_line = safe.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut characters = one_line.chars();
    let preview = characters.by_ref().take(max_chars).collect::<String>();
    if characters.next().is_some() {
        format!("{preview}…")
    } else if preview.is_empty() {
        "（空）".to_owned()
    } else {
        preview
    }
}

fn format_bytes(value: u64) -> String {
    if value < 1024 {
        format!("{value} B")
    } else if value < 1024 * 1024 {
        format!("{:.1} KiB", value as f64 / 1024.0)
    } else {
        format!("{:.1} MiB", value as f64 / 1024.0 / 1024.0)
    }
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::{Terminal, backend::TestBackend};
    use url::Url;

    use super::*;
    use crate::{
        ApiClient,
        app::{AccessMode, ConfirmAction, ConfirmState, ConflictState, Draft},
        model::{Attachment, MemoAuthor, MemoState, MemoVisibility},
    };

    fn memo(content: &str) -> Memo {
        Memo {
            id: "memo-1".to_owned(),
            content: content.to_owned(),
            visibility: MemoVisibility::Private,
            state: MemoState::Active,
            pinned: true,
            version: 2,
            created_at: 1,
            updated_at: 2,
            deleted_at: None,
            author: MemoAuthor {
                id: "user-1".to_owned(),
                name: "测试用户".to_owned(),
                username: "tester".to_owned(),
                image: None,
            },
            tags: vec!["rust".to_owned()],
            attachments: vec![Attachment {
                id: "attachment-1".to_owned(),
                filename: "设计稿.png".to_owned(),
                content_type: "image/png".to_owned(),
                size: 2048,
                status: "READY".to_owned(),
                url: "/api/v1/attachments/attachment-1/content".to_owned(),
            }],
        }
    }

    fn app() -> App {
        let client = ApiClient::with_timeout(
            Url::parse("http://127.0.0.1:9").expect("URL"),
            "cm_pat_test".to_owned(),
            Duration::from_millis(10),
        )
        .expect("client");
        let mut app = App::new(client, AccessMode::ReadWrite);
        app.app_name = "Cloud Memos 测试".to_owned();
        app.viewer_name = "测试用户".to_owned();
        app.status = "就绪".to_owned();
        app.items = vec![memo("# 标题\n\n**安全** 的正文")];
        app
    }

    fn render_app(app: &mut App, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| render(frame, app))
            .expect("render test UI");
        let buffer = terminal.backend().buffer();
        let mut output = String::new();
        for y in 0..height {
            for x in 0..width {
                output.push_str(buffer[(x, y)].symbol());
            }
            output.push('\n');
        }
        output
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect()
    }

    #[test]
    fn renders_wide_two_column_layout_and_attachment_metadata() {
        let mut app = app();
        let output = render_app(&mut app, 120, 32);
        assert!(output.contains("我的记录"));
        assert!(output.contains("Markdown详情"));
        assert!(output.contains("设计稿.png"));
        assert!(output.contains("2.0KiB"));
    }

    #[test]
    fn renders_narrow_list_and_detail_modes() {
        let mut app = app();
        let list = render_app(&mut app, 72, 24);
        assert!(list.contains("@tester"));
        assert!(!list.contains("附件（仅元数据）"));
        app.show_detail = true;
        let detail = render_app(&mut app, 72, 24);
        assert!(detail.contains("附件（仅元数据）"));
    }

    #[test]
    fn renders_empty_error_and_too_small_states() {
        let mut app = app();
        app.items.clear();
        assert!(render_app(&mut app, 72, 24).contains("还没有记录"));
        app.last_error = Some("网络不可用".to_owned());
        assert!(render_app(&mut app, 72, 24).contains("按r重试"));
        assert!(render_app(&mut app, 30, 8).contains("终端窗口太小"));
    }

    #[tokio::test]
    async fn renders_editor_confirmation_and_conflict_overlays() {
        let mut app = app();
        app.handle_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE))
            .await;
        assert!(render_app(&mut app, 100, 30).contains("Ctrl+S保存"));

        app.mode = UiMode::Confirm;
        app.confirm = Some(ConfirmState {
            prompt: "永久删除？".to_owned(),
            action: ConfirmAction::DiscardDraft,
        });
        assert!(render_app(&mut app, 100, 30).contains("永久删除？"));

        app.editor = None;
        app.mode = UiMode::Conflict;
        app.conflict = Some(ConflictState {
            local: Draft {
                memo_id: Some("memo-1".to_owned()),
                content: "本地草稿".to_owned(),
                original: "原文".to_owned(),
                visibility: MemoVisibility::Private,
                version: Some(1),
            },
            server: memo("服务端正文"),
        });
        let conflict = render_app(&mut app, 100, 30);
        assert!(conflict.contains("本地草稿"));
        assert!(conflict.contains("服务端正文"));
        assert!(conflict.contains("版本冲突"));
    }
}
