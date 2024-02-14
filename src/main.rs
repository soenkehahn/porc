use clap::Parser;
use porc_app::PorcApp;
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
    PorcApp::run(System::new(), args.pattern)
}
