use crate::{
    process::Process,
    tree::Node,
    tui_app::{self, UpdateResult},
    R,
};
use crossterm::event::{KeyCode, KeyEvent};
use nix::sys::signal::kill;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style, Stylize},
    text::Line,
    widgets::{List, ListState, Paragraph, StatefulWidget, Widget},
};
use sysinfo::{ProcessRefreshKind, System, UpdateKind};

#[derive(Debug)]
pub(crate) struct PorcApp {
    system: System,
    processes: Vec<(sysinfo::Pid, String)>,
    pattern: String,
    list_state: ListState,
    selected_pid: Option<sysinfo::Pid>,
}

impl PorcApp {
    pub(crate) fn run(system: System, pattern: Option<String>) -> R<()> {
        let app = PorcApp {
            system,
            processes: Vec::new(),
            pattern: pattern.unwrap_or("".to_string()),
            list_state: ListState::default().with_selected(Some(0)),
            selected_pid: None,
        };
        tui_app::run_ui(app)
    }
}

impl tui_app::TuiApp for PorcApp {
    fn update(&mut self, event: KeyEvent) -> R<UpdateResult> {
        let mut modifiers = event
            .modifiers
            .iter_names()
            .map(|x| x.0)
            .collect::<Vec<&str>>();
        modifiers.sort();
        match (modifiers.as_slice(), event.code, self.selected_pid) {
            (["CONTROL"], KeyCode::Char('c'), _) => {
                return Ok(UpdateResult::Exit);
            }
            ([], KeyCode::Char(key), None) if key.is_ascii() => {
                self.pattern.push(key);
            }
            ([], KeyCode::Backspace, None) => {
                self.pattern.pop();
            }
            ([], KeyCode::Up, _) => {
                self.list_state.select(Some(
                    self.list_state.selected().unwrap_or(0).saturating_sub(1),
                ));
            }
            ([], KeyCode::PageUp, _) => {
                self.list_state.select(Some(
                    self.list_state.selected().unwrap_or(0).saturating_sub(20),
                ));
            }
            ([], KeyCode::Down, _) => {
                self.list_state.select(Some(
                    self.list_state.selected().unwrap_or(0).saturating_add(1),
                ));
            }
            ([], KeyCode::PageDown, _) => {
                self.list_state.select(Some(
                    self.list_state.selected().unwrap_or(0).saturating_add(20),
                ));
            }
            ([], KeyCode::Enter, _) => {
                if let Some(selected) = self.list_state.selected() {
                    if let Some(process) = self.processes.get(selected) {
                        self.selected_pid = Some(process.0);
                    }
                }
            }
            ([], KeyCode::Esc, Some(_)) => {
                self.selected_pid = None;
            }
            ([], KeyCode::Char('t'), Some(pid)) => {
                kill(
                    nix::unistd::Pid::from_raw(pid.as_u32().try_into()?),
                    nix::sys::signal::Signal::SIGTERM,
                )?;
            }
            ([], KeyCode::Char('k'), Some(pid)) => {
                kill(
                    nix::unistd::Pid::from_raw(pid.as_u32().try_into()?),
                    nix::sys::signal::Signal::SIGKILL,
                )?;
            }
            _ => {}
        }
        let tree = Process::new_from_sysinfo(
            self.system
                .processes()
                .values()
                .filter(|process| process.thread_kind().is_none()),
        );
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
            if self.selected_pid == Some(x.0) {
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
            let status_bar = match self.selected_pid {
                None => [
                    "Ctrl+C: Quit",
                    "↑↓ : scroll",
                    "ENTER: select process",
                    &format!("type search pattern: {}", self.pattern),
                ]
                .join(" | "),
                Some(_pid) => [
                    "Ctrl+C: Quit",
                    "↑↓ : scroll",
                    "t: SIGTERM process",
                    "k: SIGKILL process",
                    "ESC: unselect & enter search mode",
                    "ENTER: select other",
                ]
                .join(" | "),
            };
            let mut status_bar = Paragraph::new(status_bar).reversed();
            if self.selected_pid.is_some() {
                status_bar = status_bar.red();
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
        self.system.refresh_processes_specifics(
            ProcessRefreshKind::new()
                .with_memory()
                .with_cpu()
                .with_exe(UpdateKind::OnlyIfNotSet),
        );
        let processes = &self.system.processes();
        if let Some(selected) = self.selected_pid {
            if !processes.keys().any(|pid| pid == &selected) {
                self.selected_pid = None;
            }
        }
        let tree = Process::new_from_sysinfo(
            processes
                .values()
                .filter(|process| process.thread_kind().is_none()),
        );
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
    use crate::porc_app::normalize_list_state;
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
}
