pub(crate) use crate::tree::Forest;
use crate::tree::Node;
use num_format::Locale;
use num_format::ToFormattedString;
use std::fmt;
use std::path::Path;
use sysinfo::Pid;
use sysinfo::ProcessRefreshKind;
use sysinfo::ThreadKind;
use sysinfo::UpdateKind;

#[derive(Debug)]
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

    fn table_header() -> String {
        "     pid   cpu       ram".to_owned()
    }

    fn table_data(&self) -> String {
        format!(
            "{:>8} {:>4.0}% {:>7}MB",
            self.pid.as_u32(),
            self.cpu,
            (self.ram / 2_u64.pow(20)).to_formatted_string(&Locale::en)
        )
    }

    fn node_header() -> String {
        "executable".to_owned()
    }

    fn parent(&self) -> Option<Pid> {
        self.parent
    }

    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match other.cpu.partial_cmp(&self.cpu) {
            Some(std::cmp::Ordering::Equal) => self.pid.cmp(&other.pid),
            Some(ordering) => ordering,
            None => self.pid.cmp(&other.pid),
        }
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

    pub(crate) fn new_process_forest(processes: &ProcessWatcher) -> Forest<Self> {
        Forest::new_forest(match processes {
            ProcessWatcher(ProcessWatcherInner::Production { system: processes }) => processes
                .processes()
                .values()
                .filter(|process| process.thread_kind() != Some(ThreadKind::Userland))
                .map(Process::from_sysinfo_process),
        })
    }
}

#[derive(Debug)]
pub(crate) struct ProcessWatcher(ProcessWatcherInner);

#[derive(Debug)]
enum ProcessWatcherInner {
    Production { system: sysinfo::System },
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
        }
    }
}
