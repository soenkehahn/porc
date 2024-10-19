pub(crate) use crate::tree::Forest;
use crate::tree::Node;
use num_format::Locale;
use num_format::ToFormattedString;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use std::fmt;
use std::iter::repeat;
use std::path::Path;
use sysinfo::Pid;
use sysinfo::ProcessRefreshKind;
use sysinfo::ThreadKind;
use sysinfo::UpdateKind;

#[derive(Debug, Clone)]
pub(crate) struct Process {
    pid: Pid,
    pub(crate) name: String,
    arguments: Vec<String>,
    parent: Option<Pid>,
    cpu: f32,
    ram: u64,
}

impl fmt::Display for Process {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.arguments.first() {
            Some(executable) => match Path::new(&executable).file_name() {
                Some(file_name) => write!(f, "{}", file_name.to_string_lossy())?,
                None => write!(f, "{}", executable)?,
            },
            None => write!(f, "{}", self.name)?,
        }
        for argument in self.arguments.iter().skip(1) {
            write!(f, " {}", argument)?;
        }
        Ok(())
    }
}

impl Node for Process {
    type Id = Pid;

    fn id(&self) -> Pid {
        self.pid
    }

    fn table_data(&self) -> String {
        format!(
            "{:>8} {:>4.0}% {:>7}MB",
            self.pid.as_u32(),
            self.cpu,
            (self.ram / 2_u64.pow(20)).to_formatted_string(&Locale::en)
        )
    }

    fn parent(&self) -> Option<Pid> {
        self.parent
    }

    fn accumulate_from(&mut self, other: &Self) {
        self.cpu += other.cpu;
        self.ram += other.ram;
    }
}

impl Process {
    fn from_sysinfo_process(process: &sysinfo::Process) -> Self {
        Process {
            pid: process.pid(),
            name: match process.exe() {
                Some(exe) => match exe.file_name() {
                    Some(file_name) => file_name.to_string_lossy().to_string(),
                    None => exe.to_string_lossy().to_string(),
                },
                None => process.name().to_string(),
            },
            arguments: process.cmd().to_vec(),
            parent: process.parent(),
            cpu: process.cpu_usage(),
            ram: process.memory(),
        }
    }

    pub(crate) fn compare(&self, other: &Process, sort_by: SortBy) -> std::cmp::Ordering {
        let ordering = match sort_by {
            SortBy::Pid => self.id().partial_cmp(&other.id()),
            SortBy::Cpu => other.cpu.partial_cmp(&self.cpu),
            SortBy::Ram => other.ram.partial_cmp(&self.ram),
        };
        match ordering {
            Some(std::cmp::Ordering::Equal) => self.pid.cmp(&other.pid),
            Some(ordering) => ordering,
            None => self.pid.cmp(&other.pid),
        }
    }

    pub(crate) fn render_header(area: Rect, sort_by: SortBy, buffer: &mut Buffer) -> u16 {
        let table_header = {
            let mut line = Line::default();
            for column in SortBy::all() {
                let leading_spaces = match column {
                    SortBy::Pid => 5,
                    SortBy::Cpu => 3,
                    SortBy::Ram => 7,
                };
                line.push_span(repeat(" ").take(leading_spaces).collect::<String>());
                line.push_span(Span::styled(
                    format!("{:?}", column).to_lowercase(),
                    if column == sort_by {
                        Style::new().add_modifier(Modifier::REVERSED)
                    } else {
                        Style::new()
                    },
                ));
            }
            line.push_span(" ");
            line
        };
        buffer.set_line(area.x, area.y, &table_header, area.width);
        if let Ok(table_header_length) = table_header.width().try_into() {
            if let Some(cell) = buffer.cell_mut((table_header_length, area.y)) {
                cell.set_symbol("┃");
            }
            buffer.set_string(
                area.x + table_header_length + 2,
                area.y,
                "executable",
                Style::new(),
            );
            for x in (area.x)..(area.width) {
                if let Some(cell) = buffer.cell_mut((x, area.y + 1)) {
                    cell.set_symbol(if x == table_header_length {
                        "╋"
                    } else {
                        "━"
                    });
                }
            }
        }
        2
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SortBy {
    Pid,
    Cpu,
    Ram,
}

impl SortBy {
    pub(crate) fn next(self) -> SortBy {
        match self {
            SortBy::Pid => SortBy::Cpu,
            SortBy::Cpu => SortBy::Ram,
            SortBy::Ram => SortBy::Pid,
        }
    }

    fn all() -> impl Iterator<Item = SortBy> {
        vec![SortBy::Pid, SortBy::Cpu, SortBy::Ram].into_iter()
    }
}

#[derive(Debug)]
pub(crate) struct ProcessWatcher(ProcessWatcherInner);

#[derive(Debug)]
enum ProcessWatcherInner {
    Production {
        system: sysinfo::System,
    },
    #[cfg(test)]
    TestWatcher {
        processes: Vec<Process>,
    },
}

impl ProcessWatcher {
    pub(crate) fn new(system: sysinfo::System) -> ProcessWatcher {
        ProcessWatcher(ProcessWatcherInner::Production { system })
    }

    pub(crate) fn refresh(&mut self) {
        match self {
            ProcessWatcher(ProcessWatcherInner::Production { system }) => system
                .refresh_processes_specifics(
                    ProcessRefreshKind::new()
                        .with_memory()
                        .with_cpu()
                        .with_cmd(UpdateKind::OnlyIfNotSet),
                ),
            #[cfg(test)]
            ProcessWatcher(ProcessWatcherInner::TestWatcher { .. }) => {}
        }
    }

    pub(crate) fn get_forest(&self) -> Forest<Process> {
        match self {
            ProcessWatcher(ProcessWatcherInner::Production { system }) => Forest::new_forest(
                system
                    .processes()
                    .values()
                    .filter(|process| process.thread_kind() != Some(ThreadKind::Userland))
                    .map(Process::from_sysinfo_process),
            ),
            #[cfg(test)]
            ProcessWatcher(ProcessWatcherInner::TestWatcher { processes }) => {
                Forest::new_forest(processes.iter().cloned())
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod test {
    use super::*;

    impl Process {
        pub(crate) fn test_process(pid: usize, cpu: f32, parent: Option<usize>) -> Process {
            Process {
                pid: pid.into(),
                name: crate::utils::test::render_number(pid).to_string(),
                arguments: Vec::new(),
                parent: parent.map(From::from),
                cpu,
                ram: 0,
            }
        }
    }

    impl ProcessWatcher {
        pub(crate) fn test_watcher(processes: Vec<Process>) -> ProcessWatcher {
            ProcessWatcher(ProcessWatcherInner::TestWatcher { processes })
        }
    }
}
