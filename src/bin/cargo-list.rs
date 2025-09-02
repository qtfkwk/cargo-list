use {
    anyhow::Result,
    cargo_list::{Crates, expanduser},
    clap::{
        Parser, ValueEnum,
        builder::{Styles, TypedValueParser},
    },
    indexmap::IndexSet,
    rayon::prelude::*,
    spinners::{Spinner, Spinners},
    sprint::*,
    std::collections::BTreeMap,
    veg::colored::{ColoredString, Colorize, Veg},
};

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

const STYLES: Styles = Styles::styled()
    .header(clap_cargo::style::HEADER)
    .usage(clap_cargo::style::USAGE)
    .literal(clap_cargo::style::LITERAL)
    .placeholder(clap_cargo::style::PLACEHOLDER)
    .error(clap_cargo::style::ERROR)
    .valid(clap_cargo::style::VALID)
    .invalid(clap_cargo::style::INVALID);

/// List and update installed crates
#[derive(Parser)]
#[command(name = "cargo", bin_name = "cargo", styles = STYLES)]
enum Cli {
    /// List and update installed crates
    List(List),
}

#[derive(clap::Args, Clone)]
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

    /**
    Cargo install metadata file
    (falls back to `~/.cargo/.crates2.json` if `$CARGO_HOME` is unset)
    */
    #[arg(
        short,
        value_name = "PATH",
        default_value = "$CARGO_HOME/.crates2.json"
    )]
    config: String,

    /// Print readme
    #[arg(short, long)]
    readme: bool,

    /// List/update crates matching given pattern(s)
    #[arg(value_name = "PATTERN")]
    include: Vec<String>,
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

    inner(&cli)
}

fn inner(cli: &List) -> Result<()> {
    let mut sp = Spinner::new(Spinners::Line, "".into());

    let installed = Crates::from_include(
        &get_config_path(&cli.config),
        &cli.include.iter().map(|x| x.as_str()).collect::<Vec<_>>(),
    )?;
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
                if cli.include.is_empty() {
                    println!("{}\n", "*No crates are installed.*".yellow().italic());
                    std::process::exit(1);
                } else {
                    println!(
                        "{}\n",
                        "*No crates matching given pattern(s) are installed.*"
                            .yellow()
                            .italic()
                    );
                    std::process::exit(2);
                }
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

            for k in &kinds {
                println!("{}\n", format!("# {k:?}").magenta().bold());
                let mut outdated = 0;
                let mut update_pinned = 0;
                let mut number = 1;
                let mut t = if cli.outdated_rust {
                    Veg::table("#|Name|Pinned|Installed|Available|Rust\n-:|-|-|-|-|-")
                } else {
                    Veg::table("#|Name|Pinned|Installed|Available\n-:|-|-|-|-")
                };
                for c in all.values().filter(|x| x.kind == *k) {
                    if *k == cargo_list::Kind::External {
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
                                available.bold(),
                                if c.outdated_rust {
                                    c.rust_version.red()
                                } else {
                                    c.rust_version.green()
                                },
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
                                c.newer[0].bold(),
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
                    } else if !cli.outdated || c.kind == cargo_list::Kind::Git {
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
                if *k == cargo_list::Kind::External {
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

            if cli.update {
                // Update external crates
                if kinds.contains(&cargo_list::Kind::External) {
                    let mut updates = outdated.clone();
                    if cli.outdated_rust {
                        updates.append(&mut outdated_rust.clone());
                    }
                    let mut update_pinned = 0;
                    if cli.ignore_req {
                        updates.append(&mut outdated_pinned.clone());
                        update_pinned += outdated_pinned.len();
                    }
                    if !updates.is_empty() {
                        println!("{}\n", "# External".magenta().bold());
                        let mut shell = Shell {
                            dry_run: cli.dry_run,
                            ..Default::default()
                        };
                        if cli.dry_run {
                            shell.info = String::from("bash");
                        }
                        for (name, c) in &updates {
                            println!("{}\n", format!("## {name:?}").yellow().bold());
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
                        let mut c = cli.clone();
                        c.update = false;
                        c.outdated = false;
                        c.include = updates.keys().map(|x| x.to_string()).collect();
                        inner(&c)?;
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

                // Update git crates
                if kinds.contains(&cargo_list::Kind::Git) {
                    let outdated = all
                        .iter()
                        .filter(|(_name, c)| c.kind == cargo_list::Kind::Git)
                        .collect::<BTreeMap<_, _>>();
                    if !outdated.is_empty() {
                        println!("{}\n", "# Git".magenta().bold());
                        let mut shell = Shell {
                            dry_run: cli.dry_run,
                            ..Default::default()
                        };
                        if cli.dry_run {
                            shell.info = String::from("bash");
                        }
                        for (name, c) in &outdated {
                            println!("{}\n", format!("## {name:?}").yellow().bold());
                            shell.run(&[Command {
                                command: c
                                    .update_command(
                                        cli.ignore_req && outdated_pinned.contains_key(*name),
                                    )
                                    .join(" "),
                                ..Default::default()
                            }]);
                        }
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

fn get_config_path(config: &str) -> std::path::PathBuf {
    if let Some(s) = config.strip_prefix("$CARGO_HOME/") {
        // Default
        if let Ok(cargo_home) = std::env::var("CARGO_HOME") {
            // $CARGO_HOME is set...
            std::path::PathBuf::from(&format!("{cargo_home}/{s}"))
        } else {
            // $CARGO_HOME is not set... fall back to the old default
            expanduser("~/.cargo/.crates2.json")
        }
    } else {
        // User provided another custom path
        std::path::PathBuf::from(config)
    }
}
