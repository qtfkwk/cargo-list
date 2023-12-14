use anyhow::Result;
use bunt::termcolor::ColorChoice;
use cargo_list::Crates;
use clap::{builder::TypedValueParser, Parser, ValueEnum};
use expanduser::expanduser;
use indexmap::IndexSet;
use is_terminal::IsTerminal;
use rayon::prelude::*;

#[cfg(unix)]
use pager::Pager;

//--------------------------------------------------------------------------------------------------

#[derive(Clone, ValueEnum)]
enum Kind {
    Local,
    Git,
    External,
}

use Kind::*;

impl Kind {
    fn into(&self) -> cargo_list::Kind {
        match self {
            External => cargo_list::Kind::External,
            Git => cargo_list::Kind::Git,
            Local => cargo_list::Kind::Local,
        }
    }
}

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
        conflicts_with = "update",
    )]
    output_format: OutputFormat,

    /// Kind(s)
    #[arg(short, value_enum, default_value = "external")]
    kind: Vec<Kind>,

    /// All kinds
    #[arg(short)]
    all_kinds: bool,

    /// Hide up-to-date crates
    #[arg(short, long)]
    outdated: bool,

    /// Update outdated crates
    #[arg(short, long, conflicts_with = "output_format")]
    update: bool,

    /// Cargo install metadata file
    #[arg(short, value_name = "PATH", default_value = "~/.cargo/.crates2.json")]
    config: String,

    /// Print readme
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
        #[cfg(unix)]
        Pager::with_pager("bat -pl md").setup();

        print!("{}", include_str!("../../README.md"));
        return Ok(());
    }

    let installed = Crates::from(&expanduser(&cli.config)?)?;
    let (all, outdated) = installed.crates();
    let crates = if cli.outdated { &outdated } else { &all };

    match cli.output_format {
        Markdown => {
            bunt::set_stdout_color_choice(if std::io::stdout().is_terminal() {
                ColorChoice::Always
            } else {
                ColorChoice::Never
            });

            if installed.is_empty() {
                bunt::println!("{$yellow+italic}*No crates are installed.*{/$}\n");
                return Ok(());
            }

            let kinds = if cli.all_kinds {
                cargo_list::ALL_KINDS.to_vec()
            } else {
                cli.kind
                    .par_iter()
                    .map(|x| x.into())
                    .collect::<IndexSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>()
            };

            for k in kinds {
                bunt::println!("{$magenta+bold}# {:?}{/$}\n", k);
                let mut outdated = 0;
                let mut external = 0;
                let mut n = 0;
                for c in all.values() {
                    if c.kind == k {
                        if k == cargo_list::Kind::External {
                            if c.outdated {
                                bunt::println!(
                                    "* {}: {[red]} => {}",
                                    c.name,
                                    c.installed,
                                    c.available
                                );
                                n += 1;
                                outdated += 1;
                            } else if !cli.outdated {
                                bunt::println!("* {}: {[green]}", c.name, c.installed);
                                n += 1;
                            }
                            external += 1;
                        } else if !cli.outdated {
                            bunt::println!("* {}: {[cyan]}", c.name, c.installed);
                            n += 1;
                        }
                    }
                }
                if n > 0 {
                    println!();
                }

                // Print a summary
                if k == cargo_list::Kind::External {
                    if outdated == 0 {
                        bunt::println!(
                            "{$green+italic}*All {} external crate{} are up-to-date!*{/$}\n",
                            external,
                            if external == 1 { "" } else { "s" },
                        );
                    } else {
                        bunt::println!(
                            "{$red+bold}**Need to update {} external crate{}!**{/$}\n",
                            outdated,
                            if outdated == 1 { "" } else { "s" },
                        );
                    }
                }
            }

            // Update crates
            if cli.update && !outdated.is_empty() {
                for c in outdated.values() {
                    if c.kind == cargo_list::Kind::External {
                        bunt::println!("```text\n$ {$bold}{}{/$}", c.update_command().join(" "));
                        c.update();
                        println!("```\n");
                    }
                }

                // Print summary
                bunt::println!(
                    "{$green+italic}*All {} external crates are up-to-date!*{/$}\n",
                    all.len(),
                );
            }
        }
        Json => {
            #[cfg(unix)]
            Pager::with_pager("bat -pl json").setup();

            println!("{}", serde_json::to_string(crates)?);
        }
        JsonPretty => {
            #[cfg(unix)]
            Pager::with_pager("bat -pl json").setup();

            println!("{}", serde_json::to_string_pretty(crates)?);
        }
        Rust => {
            #[cfg(unix)]
            Pager::with_pager("bat -pl rust").setup();

            println!("{crates:?}");
        }
        RustPretty => {
            #[cfg(unix)]
            Pager::with_pager("bat -pl rust").setup();

            println!("{crates:#?}");
        }
    }

    Ok(())
}
