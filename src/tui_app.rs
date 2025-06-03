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

pub(crate) trait TuiApp {
    fn tick(&mut self);

    fn update(&mut self, event: KeyEvent) -> R<UpdateResult>;

    fn render(&mut self, area: Rect, buf: &mut Buffer);
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum UpdateResult {
    Continue,
    Exit,
}

struct AppWrapper<T>(PhantomData<T>);

impl<T: TuiApp> StatefulWidget for &mut AppWrapper<T> {
    type State = T;

    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer, app: &mut T) {
        app.render(area, buf);
    }
}

pub(crate) fn run_ui<T: TuiApp>(app: T) -> R<()> {
    let termination_signal_received = setup_signal_handlers()?;
    stdout().execute(EnterAlternateScreen)?;
    enable_raw_mode()?;
    std::panic::set_hook(Box::new(|panic_info| {
        let _ = reset_terminal();
        eprintln!("panic: {}", panic_info);
    }));
    match main_loop(app, termination_signal_received) {
        Err(err) => {
            let _ = reset_terminal();
            Err(err)
        }
        Ok(()) => {
            reset_terminal()?;
            Ok(())
        }
    }
}

fn reset_terminal() -> R<()> {
    stdout().execute(LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}

fn main_loop<T: TuiApp>(mut app: T, termination_signal_received: Arc<AtomicBool>) -> R<()> {
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

fn redraw<T: TuiApp>(terminal: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut T) -> R<()> {
    terminal.draw(|frame| {
        frame.render_stateful_widget(&mut AppWrapper(PhantomData), frame.area(), app);
    })?;
    Ok(())
}
