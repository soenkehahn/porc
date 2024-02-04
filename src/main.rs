use std::error::Error;
use sysinfo::System;
use ui::run_ui;

mod process;
mod tree;
mod ui;

type R<A> = Result<A, Box<dyn Error>>;

fn main() -> R<()> {
    run_ui(System::new())
}
