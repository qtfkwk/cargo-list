use anyhow::Result;
use cargo_list::Crates;
use clap::{builder::TypedValueParser, Parser, ValueEnum};
use expanduser::expanduser;
use indexmap::IndexSet;
use rayon::prelude::*;
use spinners::{Spinner, Spinners};
use sprint::*;
use std::collections::BTreeMap;
use veg::colored::{ColoredString, Colorize, Veg};

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
enum Cli {
    /// List and update installed crates
    List(List),
}

#[derive(clap::Args)]
#[command(version, long_about = None, max_term_width = 80)]
struct List {
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

#[derive(Debug)]
struct Row {
    number: ColoredString,
    name: ColoredString,
    pinned: ColoredString,
    installed: ColoredString,
    available: ColoredString,
    rust: ColoredString,
    outdated_rust: bool,
}

impl Row {
    fn new(
        number: ColoredString,
        name: ColoredString,
        pinned: ColoredString,
        installed: ColoredString,
        available: ColoredString,
        rust: ColoredString,
        outdated_rust: bool,
    ) -> Box<Row> {
        Box::new(Row {
            number,
            name,
            pinned,
            installed,
            available,
            rust,
            outdated_rust,
        })
    }
}

impl veg::colored::Table for Row {
    fn row(&self) -> Vec<ColoredString> {
        let mut r = vec![
            self.number.clone(),
            self.name.clone(),
            self.pinned.clone(),
            self.installed.clone(),
            self.available.clone(),
        ];
        if self.outdated_rust {
            r.push(self.rust.clone());
        }
        r
    }
}

//--------------------------------------------------------------------------------------------------

fn main() -> Result<()> {
    let Cli::List(cli) = Cli::parse();

    if cli.readme {
        #[cfg(unix)]
        Pager::with_pager("bat -pl md").setup();

        print!("{}", include_str!("../../README.md"));
        return Ok(());
    }

    let mut sp = Spinner::new(Spinners::Line, "".into());
    let installed = Crates::from(&expanduser(&cli.config)?)?;
    sp.stop();
    eprint!("\x1b[2K\r");

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
            if installed.is_empty() {
                println!("{}\n", "*No crates are installed.*".yellow().italic());
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
                println!("{}\n", format!("# {k:?}").magenta().bold());
                let mut outdated = 0;
                let mut update_pinned = 0;
                let mut number = 1;
                let mut t = if cli.outdated_rust {
                    Veg::table("#|Name|Pinned|Installed|Available|Rust\n-:|-|-|-|-|-")
                } else {
                    Veg::table("#|Name|Pinned|Installed|Available\n-:|-|-|-|-")
                };
                for c in all.values().filter(|x| x.kind == k) {
                    if k == cargo_list::Kind::External {
                        let (pinned, available) = if let Some(pinned) = &c.version_req {
                            if c.newer.is_empty() {
                                (String::new(), c.available.to_string())
                            } else {
                                update_pinned += 1;
                                (pinned.clone(), c.newer.join(", "))
                            }
                        } else {
                            (String::new(), c.available.to_string())
                        };

                        if c.outdated {
                            t.push(Row::new(
                                number.to_string().normal(),
                                c.name.normal(),
                                pinned.normal(),
                                c.installed.red(),
                                available.normal(),
                                "".normal(),
                                cli.outdated_rust,
                            ));
                            number += 1;
                            outdated += 1;
                        } else if cli.outdated_rust {
                            if c.outdated_rust {
                                t.push(Row::new(
                                    number.to_string().normal(),
                                    c.name.normal(),
                                    pinned.normal(),
                                    c.installed.green(),
                                    "".normal(),
                                    c.rust_version.red(),
                                    cli.outdated_rust,
                                ));
                                number += 1;
                                outdated += 1;
                            } else if !cli.outdated {
                                t.push(Row::new(
                                    number.to_string().normal(),
                                    c.name.normal(),
                                    pinned.normal(),
                                    c.installed.green(),
                                    "".normal(),
                                    c.rust_version.green(),
                                    cli.outdated_rust,
                                ));
                                number += 1;
                            }
                        } else if cli.ignore_req && !c.newer.is_empty() {
                            t.push(Row::new(
                                number.to_string().normal(),
                                c.name.normal(),
                                pinned.normal(),
                                c.installed.red(),
                                c.newer[0].normal(),
                                "".normal(),
                                cli.outdated_rust,
                            ));
                            number += 1;
                            outdated += 1;
                        } else if !cli.outdated {
                            t.push(Row::new(
                                number.to_string().normal(),
                                c.name.normal(),
                                pinned.normal(),
                                c.installed.green(),
                                "".normal(),
                                "".normal(),
                                cli.outdated_rust,
                            ));
                            number += 1;
                        }
                        ext += 1;
                    } else if !cli.outdated {
                        t.push(Row::new(
                            number.to_string().normal(),
                            c.name.normal(),
                            "".normal(),
                            c.installed.cyan(),
                            "".normal(),
                            "".normal(),
                            cli.outdated_rust,
                        ));
                        number += 1;
                    }
                }

                // Print the table
                if !t.is_empty() {
                    println!("{}", t.markdown()?);
                }

                // Print a summary
                if k == cargo_list::Kind::External {
                    if outdated == 0 {
                        println!(
                            "{}\n",
                            "**All external crates are up-to-date!**".green().bold(),
                        );
                    } else {
                        println!(
                            "{}\n",
                            format!(
                                "**Need to update {} external crate{}!**",
                                outdated,
                                if outdated == 1 { "" } else { "s" }
                            )
                            .red()
                            .bold(),
                        );
                    }

                    if !cli.ignore_req && update_pinned > 0 {
                        println!(
                            "{}\n",
                            format!(
                                "*Consider updating {} pinned external crate{} via `-I`.*",
                                update_pinned,
                                if update_pinned == 1 { "" } else { "s" },
                            )
                            .yellow()
                            .italic(),
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
                    let mut shell = Shell {
                        dry_run: cli.dry_run,
                        ..Default::default()
                    };
                    if cli.dry_run {
                        shell.info = String::from("bash");
                    }
                    for (name, c) in updates {
                        shell.run(&[Command {
                            command: c
                                .update_command(
                                    cli.ignore_req && outdated_pinned.contains_key(name),
                                )
                                .join(" "),
                            ..Default::default()
                        }]);
                    }

                    // Print summary
                    if !cli.dry_run {
                        println!(
                            "{}\n",
                            format!(
                                "*All {} external crate{} are up-to-date!*",
                                ext,
                                if ext == 1 { "" } else { "s" }
                            )
                            .green()
                            .italic(),
                        );
                    }
                    if !cli.ignore_req && update_pinned > 0 {
                        println!(
                            "{}\n",
                            format!(
                                "*Consider updating {} pinned external crate{} via `-I`.*",
                                update_pinned,
                                if update_pinned == 1 { "" } else { "s" }
                            )
                            .yellow()
                            .italic(),
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
