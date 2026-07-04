//! Interactive terminal dashboard for summarizing captured carrier ledgers.
//!
//! Run with:
//!
//! ```text
//! cargo run -p nw-gridmate --example ledger_tui -- path/to/ledger.bin
//! ```

use std::{
    collections::BTreeMap,
    env,
    ffi::OsStr,
    fs, io, panic,
    path::{Path, PathBuf},
    process::ExitCode,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use nw_gridmate::{ReplaySummary, replay_ledger_file};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Tabs},
};

const USAGE: &str = "\
Usage: cargo run -p nw-gridmate --example ledger_tui -- <ledger-or-directory>...

Arguments:
  <ledger-or-directory>...  ledger.bin, *.nwdl, or directories searched recursively

Keys:
  q/Esc quit, Tab/Shift+Tab or Left/Right switch tabs, Up/Down/PageUp/PageDown/Home/End scroll
";

const TAB_TITLES: [&str; 4] = ["Overview", "Hub", "State", "Errors"];
const OVERVIEW_TAB: usize = 0;
const HUB_TAB: usize = 1;
const STATE_TAB: usize = 2;
const ERRORS_TAB: usize = 3;

static TERMINAL_ACTIVE: AtomicBool = AtomicBool::new(false);

fn main() -> ExitCode {
    match try_main() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn try_main() -> Result<(), String> {
    let args = env::args_os().skip(1).collect::<Vec<_>>();
    if args.is_empty()
        || args
            .iter()
            .any(|arg| arg == OsStr::new("-h") || arg == OsStr::new("--help"))
    {
        print!("{USAGE}");
        return Ok(());
    }

    let roots = args.into_iter().map(PathBuf::from).collect::<Vec<_>>();
    let ledgers = discover_ledgers(&roots)?;
    if ledgers.is_empty() {
        return Err(
            "no ledgers found; pass ledger.bin, *.nwdl, or a directory containing them".to_owned(),
        );
    }

    let replay = replay_ledgers(&ledgers)?;
    run_tui(App::new(replay)).map_err(|error| format!("terminal error: {error}"))
}

fn discover_ledgers(roots: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    let mut ledgers = Vec::new();
    for root in roots {
        collect_ledgers(root, &mut ledgers)?;
    }
    ledgers.sort();
    ledgers.dedup();
    Ok(ledgers)
}

fn collect_ledgers(path: &Path, ledgers: &mut Vec<PathBuf>) -> Result<(), String> {
    if !path.exists() {
        return Err(format!("path does not exist: {}", path.display()));
    }

    let metadata =
        fs::metadata(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    if metadata.is_file() {
        if is_ledger_file(path) {
            ledgers.push(path.to_path_buf());
        }
        return Ok(());
    }

    if !metadata.is_dir() {
        return Ok(());
    }

    let entries = fs::read_dir(path)
        .map_err(|error| format!("read directory {}: {error}", path.display()))?;
    for entry in entries {
        let entry = entry
            .map_err(|error| format!("read directory entry in {}: {error}", path.display()))?;
        let child = entry.path();
        let metadata = entry
            .metadata()
            .map_err(|error| format!("read {}: {error}", child.display()))?;
        if metadata.is_dir() {
            collect_ledgers(&child, ledgers)?;
        } else if metadata.is_file() && is_ledger_file(&child) {
            ledgers.push(child);
        }
    }

    Ok(())
}

fn is_ledger_file(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|name| name == OsStr::new("ledger.bin"))
        || path
            .extension()
            .is_some_and(|ext| ext == OsStr::new("nwdl"))
}

fn replay_ledgers(ledgers: &[PathBuf]) -> Result<ReplayRun, String> {
    let mut summary = ReplaySummary::default();
    let mut processed = Vec::new();

    for ledger in ledgers {
        match replay_ledger_file(ledger) {
            Ok(stats) => {
                summary.merge(stats);
                processed.push(ledger.clone());
            }
            Err(error) => {
                eprintln!("failed to replay {}: {error}", ledger.display());
            }
        }
    }

    if processed.is_empty() {
        return Err(format!(
            "failed to replay all {} discovered ledger(s)",
            ledgers.len()
        ));
    }

    Ok(ReplayRun {
        summary,
        ledgers: processed,
    })
}

struct ReplayRun {
    summary: ReplaySummary,
    ledgers: Vec<PathBuf>,
}

struct App {
    summary: ReplaySummary,
    ledgers: Vec<PathBuf>,
    active_tab: usize,
    hub_scroll: usize,
    state_scroll: usize,
    hub_page_len: usize,
    state_page_len: usize,
}

impl App {
    fn new(replay: ReplayRun) -> Self {
        Self {
            summary: replay.summary,
            ledgers: replay.ledgers,
            active_tab: OVERVIEW_TAB,
            hub_scroll: 0,
            state_scroll: 0,
            hub_page_len: 1,
            state_page_len: 1,
        }
    }

    fn source_label(&self) -> String {
        let Some(first) = self.ledgers.first() else {
            return "no ledgers".to_owned();
        };
        let first = display_name(first);
        if self.ledgers.len() == 1 {
            first
        } else {
            format!("{first} (+{} more)", self.ledgers.len() - 1)
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => true,
            KeyCode::Tab => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    self.previous_tab();
                } else {
                    self.next_tab();
                }
                false
            }
            KeyCode::BackTab | KeyCode::Left => {
                self.previous_tab();
                false
            }
            KeyCode::Right => {
                self.next_tab();
                false
            }
            KeyCode::Down => {
                self.scroll_down(1);
                false
            }
            KeyCode::Up => {
                self.scroll_up(1);
                false
            }
            KeyCode::PageDown => {
                self.scroll_down(self.active_page_len());
                false
            }
            KeyCode::PageUp => {
                self.scroll_up(self.active_page_len());
                false
            }
            KeyCode::Home => {
                self.scroll_home();
                false
            }
            KeyCode::End => {
                self.scroll_end();
                false
            }
            _ => false,
        }
    }

    fn next_tab(&mut self) {
        self.active_tab = (self.active_tab + 1) % TAB_TITLES.len();
    }

    fn previous_tab(&mut self) {
        self.active_tab = (self.active_tab + TAB_TITLES.len() - 1) % TAB_TITLES.len();
    }

    fn active_page_len(&self) -> usize {
        match self.active_tab {
            HUB_TAB => self.hub_page_len,
            STATE_TAB => self.state_page_len,
            _ => 1,
        }
        .max(1)
    }

    fn scroll_down(&mut self, amount: usize) {
        match self.active_tab {
            HUB_TAB => {
                self.hub_scroll = scroll_down(
                    self.hub_scroll,
                    self.summary.hub_types.len(),
                    self.hub_page_len,
                    amount,
                );
            }
            STATE_TAB => {
                self.state_scroll = scroll_down(
                    self.state_scroll,
                    self.summary.state_types.len(),
                    self.state_page_len,
                    amount,
                );
            }
            _ => {}
        }
    }

    fn scroll_up(&mut self, amount: usize) {
        match self.active_tab {
            HUB_TAB => {
                self.hub_scroll = self.hub_scroll.saturating_sub(amount);
            }
            STATE_TAB => {
                self.state_scroll = self.state_scroll.saturating_sub(amount);
            }
            _ => {}
        }
    }

    fn scroll_home(&mut self) {
        match self.active_tab {
            HUB_TAB => self.hub_scroll = 0,
            STATE_TAB => self.state_scroll = 0,
            _ => {}
        }
    }

    fn scroll_end(&mut self) {
        match self.active_tab {
            HUB_TAB => {
                self.hub_scroll = max_scroll(self.summary.hub_types.len(), self.hub_page_len);
            }
            STATE_TAB => {
                self.state_scroll = max_scroll(self.summary.state_types.len(), self.state_page_len);
            }
            _ => {}
        }
    }

    fn clamp_scrolls(&mut self) {
        self.hub_scroll = self
            .hub_scroll
            .min(max_scroll(self.summary.hub_types.len(), self.hub_page_len));
        self.state_scroll = self.state_scroll.min(max_scroll(
            self.summary.state_types.len(),
            self.state_page_len,
        ));
    }
}

fn scroll_down(current: usize, row_count: usize, page_len: usize, amount: usize) -> usize {
    current
        .saturating_add(amount)
        .min(max_scroll(row_count, page_len))
}

fn max_scroll(row_count: usize, page_len: usize) -> usize {
    row_count.saturating_sub(page_len.max(1))
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .and_then(OsStr::to_str)
        .map(str::to_owned)
        .unwrap_or_else(|| path.display().to_string())
}

fn run_tui(app: App) -> io::Result<()> {
    install_panic_hook();
    let _guard = TerminalGuard::enter()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;
    run_event_loop(&mut terminal, app)?;
    terminal.show_cursor()
}

fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    mut app: App,
) -> io::Result<()> {
    loop {
        terminal.draw(|frame| render(frame, &mut app))?;
        if event::poll(Duration::from_millis(200))? {
            let Event::Key(key) = event::read()? else {
                continue;
            };
            if key.kind == KeyEventKind::Press && app.handle_key(key) {
                break;
            }
        }
    }
    Ok(())
}

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        if let Err(error) = execute!(stdout, EnterAlternateScreen) {
            let _ = disable_raw_mode();
            return Err(error);
        }
        TERMINAL_ACTIVE.store(true, Ordering::SeqCst);
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore_terminal();
    }
}

fn install_panic_hook() {
    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        restore_terminal();
        default_hook(panic_info);
    }));
}

fn restore_terminal() {
    if TERMINAL_ACTIVE.swap(false, Ordering::SeqCst) {
        let mut stdout = io::stdout();
        let _ = execute!(stdout, LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}

fn render(frame: &mut Frame<'_>, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(frame.area());

    render_header(frame, app, chunks[0]);
    render_tabs(frame, app, chunks[1]);

    match app.active_tab {
        OVERVIEW_TAB => render_overview(frame, &app.summary, chunks[2]),
        HUB_TAB => render_hub(frame, app, chunks[2]),
        STATE_TAB => render_state(frame, app, chunks[2]),
        ERRORS_TAB => render_errors(frame, &app.summary, chunks[2]),
        _ => {}
    }

    render_footer(frame, chunks[3]);
}

fn render_header(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let errors = total_errors(&app.summary);
    let error_style = if errors > 0 {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::DIM)
    };
    let ledger_label = format!(
        "{} ledger{}  {}",
        app.ledgers.len(),
        if app.ledgers.len() == 1 { "" } else { "s" },
        app.source_label()
    );

    let totals = Line::from(vec![
        Span::raw(format!("records {}  ", format_count(app.summary.records))),
        Span::raw(format!(
            "datagrams {}  ",
            format_count(app.summary.datagrams)
        )),
        Span::raw(format!(
            "carrier messages {}  ",
            format_count(app.summary.carrier_messages)
        )),
        Span::raw(format!(
            "state bundle bytes {}  ",
            format_count(app.summary.state_bundle_bytes)
        )),
        Span::styled(
            format!("total errors {}", format_count(errors)),
            error_style,
        ),
    ]);
    let header = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(
                "Ledger Replay Dashboard",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(ledger_label, Style::default().fg(Color::DarkGray)),
        ]),
        totals,
    ])
    .block(Block::default().borders(Borders::ALL));
    frame.render_widget(header, area);
}

fn render_tabs(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let tabs = Tabs::new(TAB_TITLES)
        .select(app.active_tab)
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().fg(Color::DarkGray))
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .divider(" ");
    frame.render_widget(tabs, area);
}

fn render_overview(frame: &mut Frame<'_>, summary: &ReplaySummary, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(8), Constraint::Min(0)])
        .split(area);
    let top_direction = if area.width >= 76 {
        Direction::Horizontal
    } else {
        Direction::Vertical
    };
    let top = Layout::default()
        .direction(top_direction)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[0]);

    render_metric_table(
        frame,
        "Carrier",
        vec![
            ("records".to_owned(), format_count(summary.records)),
            ("datagrams".to_owned(), format_count(summary.datagrams)),
            (
                "reassembled carrier messages".to_owned(),
                format_count(summary.carrier_messages),
            ),
        ],
        top[0],
    );
    render_channel_table(frame, summary, top[1]);
    render_metric_table(
        frame,
        "State Bundles",
        vec![
            (
                "state bundles".to_owned(),
                format_count(summary.state_bundles),
            ),
            (
                "decoded bundles".to_owned(),
                format_count(summary.state_wrapped_bundles),
            ),
            (
                "bundles with replication control".to_owned(),
                format_count(summary.state_bundles_with_replication_control),
            ),
            (
                "fragments".to_owned(),
                format_count(summary.state_fragments),
            ),
            (
                "fragment decode errors".to_owned(),
                format_count(summary.state_fragment_decode_errors),
            ),
            (
                "bundle bytes".to_owned(),
                format_count(summary.state_bundle_bytes),
            ),
        ],
        chunks[1],
    );
}

fn render_metric_table(
    frame: &mut Frame<'_>,
    title: &'static str,
    rows: Vec<(String, String)>,
    area: Rect,
) {
    let rows = rows.into_iter().map(|(label, value)| {
        Row::new(vec![Cell::from(label), Cell::from(format!("{value:>14}"))])
    });
    let table = Table::new(rows, [Constraint::Min(20), Constraint::Length(16)])
        .block(Block::default().borders(Borders::ALL).title(title));
    frame.render_widget(table, area);
}

fn render_channel_table(frame: &mut Frame<'_>, summary: &ReplaySummary, area: Rect) {
    let rows = if summary.channels.is_empty() {
        vec![Row::new(vec![
            Cell::from("-"),
            Cell::from(format!("{:>12}", 0)),
        ])]
    } else {
        summary
            .channels
            .iter()
            .map(|(channel, count)| {
                Row::new(vec![
                    Cell::from(channel.to_string()),
                    Cell::from(format!("{:>12}", format_count(*count))),
                ])
            })
            .collect::<Vec<_>>()
    };
    let table = Table::new(rows, [Constraint::Length(12), Constraint::Length(14)])
        .header(header_row(vec!["channel", "messages"]))
        .block(Block::default().borders(Borders::ALL).title("Channels"));
    frame.render_widget(table, area);
}

fn render_hub(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);
    render_hub_summary(frame, &app.summary, chunks[0]);

    app.hub_page_len = table_page_len(chunks[1]);
    app.clamp_scrolls();

    if app.summary.hub_types.is_empty() {
        render_empty_block(frame, "Hub Types", "No hub message types", chunks[1]);
        return;
    }

    let rows = app
        .summary
        .hub_types_sorted()
        .into_iter()
        .skip(app.hub_scroll)
        .map(|(key, traffic)| {
            Row::new(vec![
                Cell::from(key.path),
                Cell::from(type_index_label(key.type_index)),
                Cell::from(key.name.as_str()),
                Cell::from(format!("{:>10}", format_count(traffic.count))),
                Cell::from(format!("{:>10}", format_count(traffic.bytes))),
            ])
        });
    let table = Table::new(
        rows,
        [
            Constraint::Length(24),
            Constraint::Length(12),
            Constraint::Min(20),
            Constraint::Length(12),
            Constraint::Length(12),
        ],
    )
    .header(header_row(vec![
        "path",
        "type_index",
        "name",
        "count",
        "bytes",
    ]))
    .block(Block::default().borders(Borders::ALL).title("Hub Types"));
    frame.render_widget(table, chunks[1]);
}

fn render_hub_summary(frame: &mut Frame<'_>, summary: &ReplaySummary, area: Rect) {
    let error_style = if summary.hub_parse_errors > 0 {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::DIM)
    };
    let text = Line::from(vec![
        Span::raw(format!("messages {}  ", format_count(summary.hub_messages))),
        Span::raw(format!(
            "empty {}  ",
            format_count(summary.hub_empty_envelopes)
        )),
        Span::raw(format!(
            "ambiguous {}  ",
            format_count(summary.hub_ambiguous_flows)
        )),
        Span::styled(
            format!("parse errors {}", format_count(summary.hub_parse_errors)),
            error_style,
        ),
    ]);
    frame.render_widget(
        Paragraph::new(text).block(Block::default().borders(Borders::ALL).title("Hub Summary")),
        area,
    );
}

fn render_state(frame: &mut Frame<'_>, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);
    render_state_summary(frame, &app.summary, chunks[0]);

    app.state_page_len = table_page_len(chunks[1]);
    app.clamp_scrolls();

    if app.summary.state_types.is_empty() {
        render_empty_block(frame, "State Types", "No state fragment types", chunks[1]);
        return;
    }

    let rows = app
        .summary
        .state_types_sorted()
        .into_iter()
        .skip(app.state_scroll)
        .map(|(key, traffic)| {
            Row::new(vec![
                Cell::from(type_index_label(key.type_index)),
                Cell::from(key.name.as_str()),
                Cell::from(format!("{:>10}", format_count(traffic.count))),
                Cell::from(format!("{:>10}", format_count(traffic.bytes))),
                Cell::from(format!("{:>10}", format_count(traffic.decoded))),
                Cell::from(format!("{:>10}", format_count(traffic.errors))),
            ])
        });
    let table = Table::new(
        rows,
        [
            Constraint::Length(12),
            Constraint::Min(24),
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Length(12),
        ],
    )
    .header(header_row(vec![
        "type_index",
        "name",
        "count",
        "bytes",
        "decoded",
        "errors",
    ]))
    .block(Block::default().borders(Borders::ALL).title("State Types"));
    frame.render_widget(table, chunks[1]);
}

fn render_state_summary(frame: &mut Frame<'_>, summary: &ReplaySummary, area: Rect) {
    let error_style = if summary.state_fragment_iter_errors > 0
        || summary.state_fragment_decode_errors > 0
        || summary.state_bundle_parse_errors > 0
    {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::DIM)
    };
    let text = Line::from(vec![
        Span::raw(format!("bundles {}  ", format_count(summary.state_bundles))),
        Span::raw(format!(
            "fragments {}  ",
            format_count(summary.state_fragments)
        )),
        Span::styled(
            format!(
                "iter errors {}  decode errors {}  bundle bytes {}",
                format_count(summary.state_fragment_iter_errors),
                format_count(summary.state_fragment_decode_errors),
                format_count(summary.state_bundle_bytes)
            ),
            error_style,
        ),
    ]);
    frame.render_widget(
        Paragraph::new(text).block(
            Block::default()
                .borders(Borders::ALL)
                .title("State Summary"),
        ),
        area,
    );
}

fn render_errors(frame: &mut Frame<'_>, summary: &ReplaySummary, area: Rect) {
    let tables = [
        ("Hub parse errors", &summary.hub_errors),
        ("State bundle errors", &summary.state_bundle_errors),
        ("State fragment errors", &summary.state_fragment_errors),
    ]
    .into_iter()
    .filter(|(_, errors)| !errors.is_empty())
    .collect::<Vec<_>>();

    if tables.is_empty() {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(45),
                Constraint::Length(3),
                Constraint::Percentage(55),
            ])
            .split(area);
        let message = Paragraph::new("No parse errors")
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(message, chunks[1]);
        return;
    }

    let constraints = match tables.len() {
        1 => vec![Constraint::Percentage(100)],
        2 => vec![Constraint::Percentage(50), Constraint::Percentage(50)],
        _ => vec![
            Constraint::Percentage(34),
            Constraint::Percentage(33),
            Constraint::Percentage(33),
        ],
    };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    for ((title, errors), area) in tables.into_iter().zip(chunks.iter().copied()) {
        render_error_table(frame, title, errors, area);
    }
}

fn render_error_table(
    frame: &mut Frame<'_>,
    title: &'static str,
    errors: &BTreeMap<String, usize>,
    area: Rect,
) {
    let rows = sorted_errors(errors).into_iter().map(|(error, count)| {
        Row::new(vec![
            Cell::from(format!("{:>10}", format_count(*count))),
            Cell::from(error.as_str()),
        ])
    });
    let table = Table::new(rows, [Constraint::Length(12), Constraint::Min(20)])
        .header(header_row(vec!["count", "error"]))
        .block(Block::default().borders(Borders::ALL).title(title));
    frame.render_widget(table, area);
}

fn render_empty_block(
    frame: &mut Frame<'_>,
    title: &'static str,
    message: &'static str,
    area: Rect,
) {
    let paragraph = Paragraph::new(message)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).title(title));
    frame.render_widget(paragraph, area);
}

fn render_footer(frame: &mut Frame<'_>, area: Rect) {
    let footer = Paragraph::new(
        "q/Esc quit | Tab/Shift+Tab/Left/Right tabs | Up/Down/PageUp/PageDown/Home/End scroll",
    )
    .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(footer, area);
}

fn header_row(labels: Vec<&'static str>) -> Row<'static> {
    Row::new(labels)
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .bottom_margin(1)
}

fn table_page_len(area: Rect) -> usize {
    usize::from(area.height.saturating_sub(3)).max(1)
}

fn type_index_label(type_index: Option<u32>) -> String {
    type_index
        .map(|index| index.to_string())
        .unwrap_or_else(|| "-".to_owned())
}

fn total_errors(summary: &ReplaySummary) -> usize {
    summary.hub_parse_errors
        + summary.state_bundle_parse_errors
        + summary.state_fragment_iter_errors
        + summary.state_fragment_decode_errors
}

fn sorted_errors(errors: &BTreeMap<String, usize>) -> Vec<(&String, &usize)> {
    let mut rows = errors.iter().collect::<Vec<_>>();
    rows.sort_by(|(left_error, left_count), (right_error, right_count)| {
        right_count
            .cmp(left_count)
            .then_with(|| left_error.cmp(right_error))
    });
    rows
}

fn format_count(value: usize) -> String {
    let digits = value.to_string();
    let mut formatted = String::with_capacity(digits.len() + digits.len() / 3);
    let first_group_len = digits.len() % 3;

    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (index - first_group_len).is_multiple_of(3) {
            formatted.push(',');
        }
        formatted.push(digit);
    }

    formatted
}
