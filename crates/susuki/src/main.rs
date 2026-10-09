mod cli;
mod logo;
mod output;
mod pipeline;
mod usage;

use cli::{parse_args, ArgError, Cmd};
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
    let cmd = match parse_args(std::env::args_os(), &cwd) {
        Ok(c) => c,
        Err(ArgError(m)) => {
            eprintln!("ERROR\tusage\t{m}");
            return ExitCode::from(2);
        }
    };
    match cmd {
        Cmd::Help => {
            print!("{}", usage::USAGE);
            ExitCode::SUCCESS
        }
        Cmd::Version => {
            println!("susuki {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Cmd::Run(opts) => pipeline::run(&opts),
    }
}
