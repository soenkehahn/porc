use crate::process::ProcessWatcher;
use crate::process::SortBy;
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
    processes: Vec<(sysinfo::Pid, String)>,
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
            processes: Vec::new(),
            pattern: pattern.unwrap_or("".to_string()),
            list_state: ListState::default().with_selected(Some(0)),
            ui_mode: UiMode::Normal,
            sort_column: SortBy::Cpu,
        }
    }

    pub(crate) fn run(self) -> R<()> {
        tui_app::run_ui(self)
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
                    if let Some(process) = self.processes.get(selected) {
                        self.ui_mode = UiMode::ProcessSelected(process.0);
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
        let mut tree = Process::new_process_forest(&self.process_watcher);
        tree.sort_by(&|a, b| Process::compare(a, b, self.sort_column));
        self.processes = tree.format_processes(|p| p.name.contains(&self.pattern));
        Ok(UpdateResult::Continue)
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        let header = Process::format_header(area.width.into());
        let header_len = header.len() as u16;
        Widget::render(
            List::new(header),
            Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: header_len,
            },
            buf,
        );
        let list_rect = Rect {
            x: area.x,
            y: area.y + header_len,
            width: area.width,
            height: area.height - header_len - 1,
        };
        normalize_list_state(&mut self.list_state, &self.processes, &list_rect);
        let tree_lines = self.processes.iter().map(|x| {
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
            buf,
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
                buf,
            );
        }
    }

    fn tick(&mut self) {
        self.process_watcher.refresh();
        let mut tree = Process::new_process_forest(&self.process_watcher);
        tree.sort_by(&|a, b| Process::compare(a, b, self.sort_column));
        if let UiMode::ProcessSelected(selected) = self.ui_mode {
            if !tree.iter().any(|node| node.id() == selected) {
                self.ui_mode = UiMode::Normal;
            }
        }
        self.processes = tree.format_processes(|p| p.name.contains(&self.pattern));
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
        let mut app = PorcApp::new(ProcessWatcher::test_watcher(processes), None);
        app.tick();
        app
    }

    fn render_ui(app: PorcApp) -> String {
        app.processes
            .iter()
            .map(|tuple| tuple.1.clone())
            .collect::<Vec<String>>()
            .join("\n")
    }

    #[test]
    fn processes_get_sorted_by_cpu_usage() {
        let app = test_app(vec![
            Process::test_process(1, 1.0),
            Process::test_process(2, 2.0),
            Process::test_process(3, 4.0),
            Process::test_process(4, 3.0),
        ]);
        assert_snapshot!(render_ui(app));
    }

    #[test]
    fn processes_can_be_sorted_by_pid() -> R<()> {
        let mut app = test_app(vec![
            Process::test_process(1, 1.0),
            Process::test_process(2, 2.0),
            Process::test_process(3, 4.0),
            Process::test_process(4, 3.0),
        ]);
        app.tick();
        let tab = KeyEvent {
            code: KeyCode::Tab,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };
        dbg!(&app.sort_column);
        app.update(tab)?;
        dbg!(&app.sort_column);
        app.update(tab)?;
        dbg!(&app.sort_column);
        assert_snapshot!(render_ui(app));
        Ok(())
    }
}
