use anyhow::{anyhow, Result};
use bunt::termcolor::ColorChoice;
use cargo_list::Crates;
use clap::{builder::TypedValueParser, Parser, ValueEnum};
use expanduser::expanduser;
use indexmap::IndexSet;
use is_terminal::IsTerminal;
use rayon::prelude::*;
use std::collections::BTreeMap;

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
#[command(version, long_about = None, max_term_width = 80)]
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

    /// Ignore version requirements
    #[arg(short = 'I')]
    ignore_req: bool,

    /// Consider a crate to be outdated if compiled with a Rust version
    /// different than the active toolchain
    #[arg(short = 'R')]
    outdated_rust: bool,

    /// Update outdated crates
    #[arg(short, long)]
    update: bool,

    /// Dry run
    #[arg(short = 'n', long)]
    dry_run: bool,

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
    Markdown,
    Json,
    JsonPretty,
    Rust,
    RustPretty,
}

use OutputFormat::*;

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let s = match self {
            Markdown => "md",
            Json => "json",
            JsonPretty => "json-pretty",
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
            "md" => Ok(Markdown),
            "json" => Ok(Json),
            "json-pretty" => Ok(JsonPretty),
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

    let all = installed.crates();

    let external = all
        .par_iter()
        .filter_map(|(name, c)| (c.kind == cargo_list::Kind::External).then_some((*name, *c)))
        .collect::<BTreeMap<_, _>>();
    let outdated = external
        .par_iter()
        .filter_map(|(name, c)| c.outdated.then_some((*name, *c)))
        .collect::<BTreeMap<_, _>>();
    let outdated_rust = external
        .par_iter()
        .filter_map(|(name, c)| c.outdated_rust.then_some((*name, *c)))
        .collect::<BTreeMap<_, _>>();
    let outdated_pinned = external
        .par_iter()
        .filter_map(|(name, c)| {
            if !c.outdated && !c.outdated_rust && !c.newer.is_empty() {
                Some((*name, *c))
            } else {
                None
            }
        })
        .collect::<BTreeMap<_, _>>();

    let crates = if cli.outdated {
        let mut crates = outdated.clone();
        if cli.outdated_rust {
            crates.append(&mut outdated_rust.clone());
        }
        if cli.ignore_req {
            crates.append(&mut outdated_pinned.clone());
        }
        crates
    } else {
        all.iter().map(|(name, c)| (*name, *c)).collect()
    };

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

            let mut ext = 0;
            for k in kinds {
                bunt::println!("{$magenta+bold}# {:?}{/$}\n", k);
                if k == cargo_list::Kind::External
                    && cli.outdated_rust
                    && (!cli.outdated || !outdated_rust.is_empty())
                {
                    bunt::println!(
                        "\
                            Active toolchain:\n\n```text\n{}```\n\n\
                            Active version: {[bold]}\n\
                        ",
                        installed.active_toolchain,
                        installed.active_version,
                    );
                }
                let mut outdated = 0;
                let mut update_pinned = 0;
                let mut n = 0;
                for c in all.values() {
                    if c.kind == k {
                        if k == cargo_list::Kind::External {
                            let req = if let Some(req) = &c.version_req {
                                if c.newer.is_empty() {
                                    String::new()
                                } else {
                                    update_pinned += 1;
                                    format!(" (Pinned: {req:?}; Available: {})", c.newer.join(", "))
                                }
                            } else {
                                String::new()
                            };
                            if c.outdated {
                                bunt::println!(
                                    "* {}: {[red]} => {}{}",
                                    c.name,
                                    c.installed,
                                    c.available,
                                    req,
                                );
                                n += 1;
                                outdated += 1;
                            } else if cli.outdated_rust && c.outdated_rust {
                                bunt::println!(
                                    "* {}: {[green]} (Rust: {[red]}){}",
                                    c.name,
                                    c.installed,
                                    c.rust_version,
                                    req,
                                );
                                n += 1;
                                outdated += 1;
                            } else if cli.ignore_req && !c.newer.is_empty() {
                                bunt::println!(
                                    "* {}: {[red]} => {}",
                                    c.name,
                                    c.installed,
                                    &c.newer[0],
                                );
                                n += 1;
                                outdated += 1;
                            } else if !cli.outdated {
                                bunt::println!("* {}: {[green]}{}", c.name, c.installed, req);
                                n += 1;
                            }
                            ext += 1;
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
                            "{$green+bold}**All {} external crate{} are up-to-date!**{/$}\n",
                            ext,
                            if ext == 1 { "" } else { "s" },
                        );
                    } else {
                        bunt::println!(
                            "{$red+bold}**Need to update {} external crate{}!**{/$}\n",
                            outdated,
                            if outdated == 1 { "" } else { "s" },
                        );
                    }

                    if !cli.ignore_req && update_pinned > 0 {
                        bunt::println!(
                            "\
                                {$yellow+italic}*Consider updating {} pinned external crate{} via \
                                `-I`.*{/$}\n\
                            ",
                            update_pinned,
                            if update_pinned == 1 { "" } else { "s" },
                        );
                    }
                }
            }

            // Update external crates
            if cli.update {
                let mut updates = outdated
                    .iter()
                    .map(|(name, c)| (*name, *c))
                    .collect::<BTreeMap<_, _>>();
                if cli.outdated_rust {
                    updates.append(&mut outdated_rust.clone());
                }
                let mut update_pinned = 0;
                if cli.ignore_req {
                    updates.append(&mut outdated_pinned.clone());
                    update_pinned += outdated_pinned.len();
                }
                if !updates.is_empty() {
                    for (name, c) in &updates {
                        let command =
                            c.update_command(cli.ignore_req && outdated_pinned.contains_key(name));
                        if cli.dry_run {
                            bunt::println!("```bash\n{$bold}{}{/$}", command.join(" "));
                        } else {
                            bunt::println!("```text\n$ {$bold}{}{/$}", command.join(" "));
                            run(&command)?;
                        }
                        println!("```\n");
                    }

                    // Print summary
                    if !cli.dry_run {
                        bunt::println!(
                            "{$green+italic}*All {} external crate{} are up-to-date!*{/$}\n",
                            ext,
                            if ext == 1 { "" } else { "s" },
                        );
                    }
                    if !cli.ignore_req && update_pinned > 0 {
                        bunt::println!(
                            "\
                                {$yellow+italic}*Consider updating {} pinned external crate{} via \
                                `-I`.*{/$}\n\
                            ",
                            update_pinned,
                            if update_pinned == 1 { "" } else { "s" },
                        );
                    }
                }
            }
        }
        Json => {
            #[cfg(unix)]
            Pager::with_pager("bat -pl json").setup();

            println!("{}", serde_json::to_string(&crates)?);
        }
        JsonPretty => {
            #[cfg(unix)]
            Pager::with_pager("bat -pl json").setup();

            println!("{}", serde_json::to_string_pretty(&crates)?);
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

//--------------------------------------------------------------------------------------------------

/**
Run a command
*/
pub fn run(command: &[String]) -> Result<()> {
    if std::process::Command::new(&command[0])
        .args(&command[1..])
        .spawn()
        .unwrap()
        .wait()
        .unwrap()
        .success()
    {
        Ok(())
    } else {
        Err(anyhow!("Command failed!"))
    }
}
