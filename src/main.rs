use crate::porc_app::PorcApp;
use crate::process::ProcessWatcher;
use clap::Parser;
use std::error::Error;
use sysinfo::System;

mod porc_app;
mod process;
mod tree;
mod tui_app;

type R<A> = Result<A, Box<dyn Error>>;

#[derive(Parser, Debug)]
struct Args {
    #[arg(help = "search pattern for filtering the process tree")]
    pattern: Option<String>,
}

fn main() -> R<()> {
    let args = Args::parse();
    PorcApp::run(PorcApp::new(
        ProcessWatcher::new(System::new()),
        args.pattern,
    ))
}
