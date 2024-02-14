use crate::porc_app::PorcApp;
use crate::process::ProcessWatcher;
use clap::builder::ValueParser;
use clap::Parser;
use regex::Regex;
use std::error::Error;
use sysinfo::System;

mod porc_app;
mod process;
mod tree;
mod tui_app;
mod utils;

type R<A> = Result<A, Box<dyn Error + Send + Sync>>;

#[derive(Parser, Debug)]
struct Args {
    #[
        arg(
            help = "search regex for filtering the process tree (case-insensitive)",
            value_parser = ValidatedRegexString::value_parser()
        )
    ]
    regex: Option<ValidatedRegexString>,
}

#[derive(Debug, Clone)]
struct ValidatedRegexString(String);

impl Default for ValidatedRegexString {
    fn default() -> Self {
        Self("".to_string())
    }
}

impl ValidatedRegexString {
    fn new(regex: &str) -> R<Self> {
        Regex::new(regex)?;
        Ok(ValidatedRegexString(regex.to_string()))
    }

    fn value_parser() -> ValueParser {
        ValueParser::new(|arg: &str| -> R<ValidatedRegexString> { ValidatedRegexString::new(arg) })
    }
}

fn main() -> R<()> {
    let args = Args::parse();
    PorcApp::run(PorcApp::new(
        ProcessWatcher::new(System::new()),
        args.regex,
    )?)
}
