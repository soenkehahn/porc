use self::app::UpdateResult;
use crate::{process::Process, tree::Node, R};
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

pub(crate) fn run_ui(system: System) -> R<()> {
    app::run_ui(PorcApp::new(system))
}

#[derive(Debug)]
struct PorcApp {
    system: System,
    processes: Vec<(sysinfo::Pid, String)>,
    pattern: String,
    list_state: ListState,
    selected_pid: Option<sysinfo::Pid>,
}

impl PorcApp {
    fn new(system: System) -> Self {
        PorcApp {
            system,
            processes: Vec::new(),
            pattern: "".to_string(),
            list_state: ListState::default().with_selected(Some(0)),
            selected_pid: None,
        }
    }
}

impl app::App for PorcApp {
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
                        self.selected_pid = process.0.try_into()?;
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
        let status_bar = match self.selected_pid {
            None => format!(
                "Ctrl+C: Quit | ↑↓ : scroll | ENTER: select process | type search pattern: {}",
                self.pattern
            ),
            Some(_pid) => {
                "Ctrl+C: Quit | ↑↓ : scroll | t: SIGTERM process | k: SIGKILL process | ESC: unselect & enter search mode | ENTER: select other".to_string()
            }
        };
        Paragraph::new(status_bar).black().on_white().render(
            Rect {
                x: area.x,
                y: area.height - 1,
                width: area.width,
                height: 1,
            },
            buf,
        );
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
    use crate::ui::normalize_list_state;
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

mod app {
    use crate::R;
    use crossterm::{
        event::{self, KeyEvent, KeyEventKind},
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
        ExecutableCommand,
    };
    use ratatui::{
        buffer::Buffer,
        layout::Rect,
        prelude::{CrosstermBackend, Terminal},
        widgets::StatefulWidget,
    };
    use std::{
        io::stdout,
        marker::PhantomData,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
        time::Instant,
    };
    use std::{io::Stdout, time::Duration};

    pub(crate) trait App {
        fn tick(&mut self);

        fn update(&mut self, event: KeyEvent) -> R<UpdateResult>;

        fn render(&mut self, area: Rect, buf: &mut Buffer);
    }

    pub(crate) enum UpdateResult {
        Continue,
        Exit,
    }

    struct AppWrapper<T>(PhantomData<T>);

    impl<T: App> StatefulWidget for &mut AppWrapper<T> {
        type State = T;

        fn render(
            self,
            area: ratatui::prelude::Rect,
            buf: &mut ratatui::prelude::Buffer,
            app: &mut T,
        ) {
            app.render(area, buf);
        }
    }

    pub(crate) fn run_ui<T: App>(mut app: T) -> R<()> {
        let termination_signal_received = setup_signal_handlers()?;
        stdout().execute(EnterAlternateScreen)?;
        enable_raw_mode()?;
        std::panic::set_hook(Box::new(|panic_info| {
            let _ = stdout().execute(LeaveAlternateScreen);
            let _ = disable_raw_mode();
            eprintln!("panic: {}", panic_info);
        }));
        let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
        terminal.clear()?;
        let tick_length = Duration::from_millis(1000);
        let mut last_tick = Instant::now();
        app.tick();
        redraw(&mut terminal, &mut app)?;
        loop {
            if termination_signal_received.load(Ordering::Relaxed) {
                break;
            }
            let has_event = event::poll(
                tick_length
                    .checked_sub(last_tick.elapsed())
                    .unwrap_or_default(),
            )?;
            if has_event {
                let event = event::read()?;
                if let event::Event::Key(key) = event {
                    if key.kind == KeyEventKind::Press {
                        match app.update(key)? {
                            UpdateResult::Continue => {}
                            UpdateResult::Exit => break,
                        }
                    }
                }
            } else {
                app.tick();
                last_tick = Instant::now();
            }
            redraw(&mut terminal, &mut app)?;
        }
        stdout().execute(LeaveAlternateScreen)?;
        disable_raw_mode()?;
        Ok(())
    }

    fn setup_signal_handlers() -> R<Arc<AtomicBool>> {
        use signal_hook::consts::{SIGINT, SIGTERM};
        use signal_hook::flag::register;
        let result = Arc::new(AtomicBool::new(false));
        register(SIGTERM, Arc::clone(&result))?;
        register(SIGINT, Arc::clone(&result))?;
        Ok(result)
    }

    fn redraw<T: App>(terminal: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut T) -> R<()> {
        terminal.draw(|frame| {
            frame.render_stateful_widget(&mut AppWrapper(PhantomData), frame.size(), app);
        })?;
        Ok(())
    }
}
