use anyhow::Result;
use cargo_list::Crates;
use clap::{builder::TypedValueParser, Parser};

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
    )]
    output_format: OutputFormat,

    /// Hide up-to-date crates
    #[arg(long)]
    outdated: bool,

    /// Update outdated crates
    #[arg(long, conflicts_with = "output_format")]
    update: bool,
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
    let output_format = if cli.update {
        Markdown
    } else {
        cli.output_format
    };
    let installed = Crates::new()?;
    match output_format {
        Json => {
            println!("{}", serde_json::to_string(installed.crates(cli.outdated))?);
        }
        JsonPretty => {
            println!("{}", serde_json::to_string_pretty(installed.crates(cli.outdated))?);
        }
        Markdown => {
            if installed.all.is_empty() {
                bunt::println!("{$yellow+italic}*No crates are installed.*{/$}\n");
                return Ok(());
            }
            if cli.outdated {
                if installed.outdated.is_empty() {
                    bunt::println!("{$green+italic}*All crates are up-to-date!*{/$}\n");
                    return Ok(());
                } else {
                    for c in &installed.outdated {
                        bunt::println!("* {}: {[red]} => {}", c.name, c.installed, c.available);
                    }
                }
            } else {
                for c in &installed.all {
                    if c.outdated {
                        bunt::println!("* {}: {[red]} => {}", c.name, c.installed, c.available);
                    } else {
                        bunt::println!("* {}: {[green]}", c.name, c.installed);
                    }
                }
                if installed.outdated.is_empty() {
                    bunt::println!("\n{$green+italic}*All crates are up-to-date!*{/$}\n");
                    return Ok(());
                }
            }
            println!();
            if cli.update {
                if installed.outdated.is_empty() {
                    bunt::println!("{$green+italic}*All crates are up-to-date!*{/$}\n");
                    return Ok(());
                } else {
                    for c in &installed.outdated {
                        bunt::println!("```text\n$ {$bold}cargo install {}{/$}", c.name);
                        c.update();
                        println!("```\n");
                    }
                }
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
