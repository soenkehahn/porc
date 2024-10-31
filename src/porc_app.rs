use crate::process::ProcessWatcher;
use crate::process::SortBy;
use crate::tree::Forest;
use crate::{
    process::Process,
    tree::Node,
    tui_app::{self, UpdateResult},
    R,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use nix::sys::signal::kill;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style, Stylize},
    text::Line,
    widgets::{List, ListState, Paragraph, StatefulWidget, Widget},
};

#[derive(Debug)]
pub(crate) struct PorcApp {
    process_watcher: ProcessWatcher,
    forest: Forest<Process>,
    pattern: String,
    list_state: ListState,
    ui_mode: UiMode,
    sort_column: SortBy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UiMode {
    Normal,
    EditingPattern,
    ProcessSelected(sysinfo::Pid),
}

impl PorcApp {
    pub(crate) fn new(process_watcher: ProcessWatcher, pattern: Option<String>) -> PorcApp {
        PorcApp {
            process_watcher,
            forest: Forest::empty(),
            pattern: pattern.unwrap_or("".to_string()),
            list_state: ListState::default().with_selected(Some(0)),
            ui_mode: UiMode::Normal,
            sort_column: SortBy::default(),
        }
    }

    pub(crate) fn run(self) -> R<()> {
        tui_app::run_ui(self)
    }

    fn update_processes(&mut self) {
        self.forest = self.process_watcher.get_forest();
        self.forest
            .sort_by(&|a, b| Process::compare(a, b, self.sort_column));
        self.forest.filter(|p| p.name.contains(&self.pattern));
        if let UiMode::ProcessSelected(selected) = self.ui_mode {
            if !self.forest.iter().any(|node| node.id() == selected) {
                self.ui_mode = UiMode::Normal;
            }
        }
    }
}

impl tui_app::TuiApp for PorcApp {
    fn update(&mut self, event: KeyEvent) -> R<UpdateResult> {
        match (event.modifiers, self.ui_mode, event.code) {
            (KeyModifiers::CONTROL, _, KeyCode::Char('c'))
            | (KeyModifiers::NONE, UiMode::Normal, KeyCode::Char('q')) => {
                return Ok(UpdateResult::Exit);
            }
            (KeyModifiers::NONE, _, KeyCode::Up) => {
                self.list_state.select(Some(
                    self.list_state.selected().unwrap_or(0).saturating_sub(1),
                ));
            }
            (KeyModifiers::NONE, _, KeyCode::PageUp) => {
                self.list_state.select(Some(
                    self.list_state.selected().unwrap_or(0).saturating_sub(20),
                ));
            }
            (KeyModifiers::NONE, _, KeyCode::Down) => {
                self.list_state.select(Some(
                    self.list_state.selected().unwrap_or(0).saturating_add(1),
                ));
            }
            (KeyModifiers::NONE, _, KeyCode::PageDown) => {
                self.list_state.select(Some(
                    self.list_state.selected().unwrap_or(0).saturating_add(20),
                ));
            }
            (KeyModifiers::NONE, _, KeyCode::Enter) => {
                if let Some(selected) = self.list_state.selected() {
                    if let Some(process) = self.forest.0.get(selected) {
                        self.ui_mode = UiMode::ProcessSelected(process.node.id());
                    }
                }
            }
            (KeyModifiers::NONE, _, KeyCode::Char('/')) => {
                self.ui_mode = UiMode::EditingPattern;
            }
            (KeyModifiers::NONE, _, KeyCode::Tab) => {
                self.sort_column = self.sort_column.next();
            }

            // mode specific actions
            (
                KeyModifiers::NONE,
                UiMode::EditingPattern | UiMode::ProcessSelected(_),
                KeyCode::Esc,
            ) => {
                self.ui_mode = UiMode::Normal;
            }
            (KeyModifiers::NONE, UiMode::EditingPattern, KeyCode::Char(key)) if key.is_ascii() => {
                self.pattern.push(key);
            }
            (KeyModifiers::NONE, UiMode::EditingPattern, KeyCode::Backspace) => {
                self.pattern.pop();
            }
            (KeyModifiers::NONE, UiMode::ProcessSelected(pid), KeyCode::Char('t')) => {
                kill(
                    nix::unistd::Pid::from_raw(pid.as_u32().try_into()?),
                    nix::sys::signal::Signal::SIGTERM,
                )?;
            }
            (KeyModifiers::NONE, UiMode::ProcessSelected(pid), KeyCode::Char('k')) => {
                kill(
                    nix::unistd::Pid::from_raw(pid.as_u32().try_into()?),
                    nix::sys::signal::Signal::SIGKILL,
                )?;
            }
            _ => {}
        }
        self.update_processes();
        Ok(UpdateResult::Continue)
    }

    fn render(&mut self, area: Rect, buffer: &mut Buffer) {
        let header_height = Process::render_header(area, self.sort_column, buffer);
        let list_rect = Rect {
            x: area.x,
            y: area.y + header_height,
            width: area.width,
            height: area.height - header_height - 1,
        };
        let list = self.forest.format_processes();
        normalize_list_state(&mut self.list_state, &list, &list_rect);
        let tree_lines = list.iter().map(|x| {
            let line = Line::raw(x.1.as_str());
            if self.ui_mode == UiMode::ProcessSelected(x.0) {
                line.patch_style(Color::Red)
            } else {
                line
            }
        });
        StatefulWidget::render(
            List::new(tree_lines).highlight_style(Style::new().add_modifier(Modifier::REVERSED)),
            list_rect,
            buffer,
            &mut self.list_state,
        );
        {
            let status_bar = match self.ui_mode {
                UiMode::Normal => {
                    let mut commands = vec![
                        "Ctrl+C: Quit".to_string(),
                        "↑↓ : scroll".to_string(),
                        "ENTER: select process".to_string(),
                        "/: filter processes".to_string(),
                    ];
                    if !self.pattern.is_empty() {
                        commands.push(format!("search pattern: {}", self.pattern));
                    }
                    commands.join(" | ")
                }
                UiMode::EditingPattern => [
                    "Ctrl+C: Quit",
                    "↑↓ : scroll",
                    "ENTER: select process",
                    "ESC: exit search mode",
                    &format!("type search pattern: {}▌", self.pattern),
                ]
                .join(" | "),
                UiMode::ProcessSelected(_pid) => {
                    let mut commands = vec![
                        "Ctrl+C: Quit".to_string(),
                        "↑↓ : scroll".to_string(),
                        "t: SIGTERM process".to_string(),
                        "k: SIGKILL process".to_string(),
                        "ESC: unselect".to_string(),
                        "ENTER: select other".to_string(),
                    ];
                    if !self.pattern.is_empty() {
                        commands.push(format!("search pattern: {}", self.pattern));
                    }
                    commands.join(" | ")
                }
            };
            let mut status_bar = Paragraph::new(status_bar).reversed();
            match self.ui_mode {
                UiMode::Normal => {}
                UiMode::EditingPattern => {
                    status_bar = status_bar.yellow();
                }
                UiMode::ProcessSelected(_) => {
                    status_bar = status_bar.red();
                }
            }
            status_bar.render(
                Rect {
                    x: area.x,
                    y: area.height - 1,
                    width: area.width,
                    height: 1,
                },
                buffer,
            );
        }
    }

    fn tick(&mut self) {
        self.process_watcher.refresh();
        self.update_processes();
    }
}

fn normalize_list_state<T>(list_state: &mut ListState, list: &Vec<T>, rect: &Rect) {
    match list_state.selected_mut() {
        Some(ref mut selected) => {
            *selected = (*selected).min(list.len().saturating_sub(1));
        }
        None => {}
    }
    *list_state.offset_mut() = list_state
        .offset()
        .min(list.len().saturating_sub(rect.height.into()));
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::tui_app::TuiApp;
    use crossterm::event::{KeyEventKind, KeyEventState};
    use insta::assert_snapshot;
    use ratatui::buffer::Cell;
    use ratatui::layout::Rect;
    use ratatui::widgets::ListState;

    const RECT: Rect = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 20,
    };

    #[test]
    fn normalize_leaves_state_unmodified() {
        let mut list_state = ListState::default().with_selected(Some(7)).with_offset(5);
        normalize_list_state(&mut list_state, &vec![(); 30], &RECT);
        assert_eq!(list_state.selected(), Some(7));
        assert_eq!(list_state.offset(), 5);
    }

    #[test]
    fn normalize_caps_at_the_list_end() {
        let mut list_state = ListState::default().with_selected(Some(11));
        normalize_list_state(&mut list_state, &vec![(); 10], &RECT);
        assert_eq!(list_state.selected(), Some(9));
    }

    #[test]
    fn normalize_resets_offset_to_zero_when_the_list_fits_the_area() {
        let mut list_state = ListState::default().with_selected(Some(0)).with_offset(5);
        normalize_list_state(&mut list_state, &vec![(); 10], &RECT);
        assert_eq!(list_state.offset(), 0);
    }

    #[test]
    fn normalize_scrolls_up_when_offset_is_too_big() {
        let mut list_state = ListState::default().with_selected(Some(0)).with_offset(25);
        normalize_list_state(&mut list_state, &vec![(); 30], &RECT);
        assert_eq!(list_state.offset(), 10);
    }

    fn test_app(processes: Vec<Process>) -> PorcApp {
        let mut app = PorcApp::new(ProcessWatcher::fake(processes), None);
        app.tick();
        app
    }

    fn render_ui(mut app: PorcApp) -> String {
        let area = Rect::new(0, 0, 80, 10);
        let mut buffer = Buffer::filled(area, Cell::new(" "));
        app.render(area, &mut buffer);
        let mut result = String::new();
        for y in 0..area.height {
            for x in 0..area.width {
                let symbol = buffer[(x, y)].symbol();
                let symbol = if buffer[(x, y)].modifier.contains(Modifier::REVERSED) {
                    crate::utils::test::underline(symbol)
                } else {
                    symbol.to_string()
                };
                result.push_str(&symbol);
            }
            result.push('\n')
        }
        result
    }

    #[test]
    fn shows_a_tree_with_header_and_side_columns() {
        let app = test_app(vec![
            Process::fake(1, 4.0, None),
            Process::fake(2, 3.0, Some(1)),
            Process::fake(3, 2.0, Some(2)),
            Process::fake(4, 1.0, None),
            Process::fake(5, 0.0, Some(4)),
        ]);
        assert_snapshot!(render_ui(app));
    }

    #[test]
    fn processes_get_sorted_by_pid() {
        let app = test_app(vec![
            Process::fake(1, 1.0, None),
            Process::fake(2, 2.0, None),
            Process::fake(3, 4.0, None),
            Process::fake(4, 3.0, None),
        ]);
        assert_snapshot!(render_ui(app));
    }

    #[test]
    fn processes_can_be_sorted_by_cpu() -> R<()> {
        let mut app = test_app(vec![
            Process::fake(1, 1.0, None),
            Process::fake(2, 2.0, None),
            Process::fake(3, 4.0, None),
            Process::fake(4, 3.0, None),
        ]);
        app.update(KeyEvent {
            code: KeyCode::Tab,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        })?;
        assert_snapshot!(render_ui(app));
        Ok(())
    }

    #[test]
    fn more_complicated_tree() -> R<()> {
        let app = test_app(vec![
            Process::fake(1, 1.0, None),
            Process::fake(2, 2.0, Some(1)),
            Process::fake(3, 3.0, Some(2)),
            Process::fake(4, 4.0, Some(1)),
            Process::fake(5, 5.0, Some(4)),
            Process::fake(6, 5.0, Some(4)),
            Process::fake(7, 5.0, Some(6)),
        ]);
        assert_snapshot!(render_ui(app));
        Ok(())
    }

    #[test]
    fn filtering() -> R<()> {
        let mut app = test_app(vec![
            Process::fake(1, 1.0, None),
            Process::fake(2, 2.0, Some(1)),
            Process::fake(3, 3.0, Some(2)),
            Process::fake(4, 4.0, Some(1)),
            Process::fake(5, 5.0, Some(4)),
            Process::fake(6, 5.0, Some(4)),
            Process::fake(7, 5.0, Some(6)),
        ]);
        app.pattern = "four".to_owned();
        app.tick();
        assert_snapshot!(render_ui(app));
        Ok(())
    }
}
