use anyhow::Result;
use bunt::termcolor::ColorChoice;
use cargo_list::Crates;
use clap::{builder::TypedValueParser, Parser};
use is_terminal::IsTerminal;
use pager::Pager;

//--------------------------------------------------------------------------------------------------

/// List and update installed crates
#[derive(Parser)]
#[command(name = "cargo")]
#[command(bin_name = "cargo")]
enum Command {
    /// List and update installed crates
    List(Cli),
}

#[derive(clap::Args)]
#[command(version, long_about = None)]
struct Cli {
    /// Output format
    #[arg(
        short = 'f',
        value_name = "FORMAT",
        default_value_t = Markdown,
        value_parser = clap::builder::PossibleValuesParser::new(
            ["json", "json-pretty", "md", "rust", "rust-pretty"],
        ).map(|s| s.parse::<OutputFormat>().unwrap()),
        conflicts_with_all = ["update", "update_all"],
    )]
    output_format: OutputFormat,

    /// Hide up-to-date crates
    #[arg(long)]
    outdated: bool,

    /// Update outdated crates
    #[arg(long, conflicts_with_all = ["output_format", "update_all"])]
    update: bool,

    /// Force reinstall all crates
    #[arg(long)]
    update_all: bool,

    /// Print the readme
    #[arg(short, long)]
    readme: bool,
}

//--------------------------------------------------------------------------------------------------

#[derive(Clone)]
enum OutputFormat {
    Json,
    JsonPretty,
    Markdown,
    Rust,
    RustPretty,
}

use OutputFormat::*;

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let s = match self {
            Json => "json",
            JsonPretty => "json-pretty",
            Markdown => "md",
            Rust => "rust",
            RustPretty => "rust-pretty",
        };
        s.fmt(f)
    }
}

impl std::str::FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "json" => Ok(Json),
            "json-pretty" => Ok(JsonPretty),
            "md" => Ok(Markdown),
            "rust" => Ok(Rust),
            "rust-pretty" => Ok(RustPretty),
            _ => Err(format!("Unknown output format: {s}")),
        }
    }
}

//--------------------------------------------------------------------------------------------------

fn main() -> Result<()> {
    let Command::List(cli) = Command::parse();
    if cli.readme {
        let readme = include_str!("../../README.md");
        Pager::with_pager("bat -pl md").setup();
        print!("{}", readme);
        return Ok(());
    }
    let installed = Crates::new()?;
    match cli.output_format {
        Json => {
            println!("{}", serde_json::to_string(installed.crates(cli.outdated))?);
        }
        JsonPretty => {
            println!(
                "{}",
                serde_json::to_string_pretty(installed.crates(cli.outdated))?
            );
        }
        Markdown => {
            bunt::set_stdout_color_choice(if std::io::stdout().is_terminal() {
                ColorChoice::Always
            } else {
                ColorChoice::Never
            });
            if installed.all.is_empty() {
                bunt::println!("{$yellow+italic}*No crates are installed.*{/$}\n");
                return Ok(());
            }
            let mut n = 0;
            for c in &installed.all {
                if c.outdated {
                    bunt::println!("* {}: {[red]} => {}", c.name, c.installed, c.available);
                    n += 1;
                } else if !cli.outdated {
                    bunt::println!("* {}: {[green]}", c.name, c.installed);
                    n += 1;
                }
            }
            if n > 0 {
                println!();
            }
            let installed = if cli.update && !installed.outdated.is_empty() {
                for c in &installed.outdated {
                    bunt::println!("```text\n$ {$bold}cargo install {}{/$}", c.name);
                    c.update();
                    println!("```\n");
                }
                for c in &installed.outdated {
                    bunt::println!("* {}: {} => {[green]}", c.name, c.installed, c.available);
                }
                println!();
                Crates::new()?
            } else if cli.update_all {
                for c in &installed.all {
                    bunt::println!("```text\n$ {$bold}cargo install --force {}{/$}", c.name);
                    c.update_force();
                    println!("```\n");
                }
                Crates::new()?
            } else {
                installed
            };
            if installed.outdated.is_empty() {
                bunt::println!("{$green+italic}*All crates are up-to-date!*{/$}\n");
            }
        }
        Rust => {
            println!("{:?}", installed.crates(cli.outdated));
        }
        RustPretty => {
            println!("{:#?}", installed.crates(cli.outdated));
        }
    }
    Ok(())
}
