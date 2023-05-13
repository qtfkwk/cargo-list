use anyhow::Result;
use cargo_list::Crates;
use clap::{builder::TypedValueParser, Parser};
use colored::*;

/// Improved `cargo install --list`
#[derive(Parser)]
#[command(version, max_term_width = 80)]
struct Cli {
    /// Output format
    #[arg(
        short = 'f',
        value_name = "FORMAT",
        default_value_t = Markdown,
        value_parser = clap::builder::PossibleValuesParser::new(
            ["json", "json-pretty", "md", "rust", "rust-pretty"],
        ).map(|s| s.parse::<OutputFormat>().unwrap()),
    )]
    output_format: OutputFormat,

    /// Hide up-to-date crates
    #[arg(long)]
    outdated: bool,

    /// Update outdated crates
    #[arg(long, conflicts_with = "output_format")]
    update: bool,
}

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

fn main() -> Result<()> {
    let cli = Cli::parse();
    let output_format = if cli.update {
        Markdown
    } else {
        cli.output_format
    };
    let installed = Crates::new()?;
    match output_format {
        Json => {
            if cli.outdated {
                println!("{}", serde_json::to_string(&installed.outdated)?);
            } else {
                println!("{}", serde_json::to_string(&installed.all)?);
            }
        }
        JsonPretty => {
            if cli.outdated {
                println!("{}", serde_json::to_string_pretty(&installed.outdated)?);
            } else {
                println!("{}", serde_json::to_string_pretty(&installed.all)?);
            }
        }
        Markdown => {
            println!("{}\n", "# cargo-list".bold());
            if installed.all.is_empty() {
                println!("{}\n", "*No crates are installed.*".yellow().italic());
                return Ok(());
            }
            if cli.outdated {
                if installed.outdated.is_empty() {
                    println!("{}\n", "*All crates are up-to-date!*".green().italic());
                    return Ok(());
                } else {
                    for c in &installed.outdated {
                        println!("* {}: {} => {}", c.name, c.installed.red(), c.available);
                    }
                }
            } else {
                for c in &installed.all {
                    if c.outdated {
                        println!("* {}: {} => {}", c.name, c.installed.red(), c.available);
                    } else {
                        println!("* {}: {}", c.name, c.installed.green());
                    }
                }
                if installed.outdated.is_empty() {
                    println!("\n{}\n", "*All crates are up-to-date!*".green().italic());
                    return Ok(());
                }
            }
            println!();
            if cli.update {
                println!("{}\n", "## Update".bold());
                if installed.outdated.is_empty() {
                    println!("{}\n", "*All crates are up-to-date!*".green().italic());
                    return Ok(());
                } else {
                    for c in &installed.outdated {
                        println!("```text\n$ {}", format!("cargo install {}", c.name).bold());
                        c.update();
                        println!("```\n");
                    }
                    println!();
                }
            }
        }
        Rust => {
            println!(
                "{:?}",
                if cli.outdated {
                    installed.outdated
                } else {
                    installed.all
                }
            );
        }
        RustPretty => {
            println!(
                "{:#?}",
                if cli.outdated {
                    installed.outdated
                } else {
                    installed.all
                }
            );
        }
    }
    Ok(())
}
