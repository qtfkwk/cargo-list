//! CLI

//--------------------------------------------------------------------------------------------------
// Crates

use {cargo_list::*, clap::Parser, veg::colored::Colorize};

//--------------------------------------------------------------------------------------------------
// Functions

fn main() {
    env_logger::init();

    match Command::parse() {
        Command::List(mut list) => {
            if let Err(e) = list.run() {
                eprintln!("{}", format!("Error: {e}").red().bold());
                std::process::exit(
                    e.downcast_ref::<CargoListError>()
                        .map_or(1, |e| e.exit_code),
                );
            }
        }
    }
}
