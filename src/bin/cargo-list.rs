//--------------------------------------------------------------------------------------------------
// Crates

use {
    anyhow::{Result, anyhow},
    cargo_list::{CrateOptions, Crates, expanduser},
    clap::{Parser, ValueEnum, builder::TypedValueParser},
    clap_cargo::style::CLAP_STYLING,
    indexmap::IndexSet,
    rayon::prelude::*,
    spinners::{Spinner, Spinners},
    sprint::{Command, Shell},
    std::{
        collections::{BTreeMap, BTreeSet},
        path::PathBuf,
    },
    veg::colored::{ColoredString, Colorize, Veg},
};

#[cfg(unix)]
use pager2::Pager;

//--------------------------------------------------------------------------------------------------
// Enums

#[derive(Clone, ValueEnum)]
enum Kind {
    Local,
    Git,
    External,
}

use Kind::{External, Git, Local};

impl Kind {
    fn into(&self) -> cargo_list::Kind {
        match self {
            External => cargo_list::Kind::External,
            Git => cargo_list::Kind::Git,
            Local => cargo_list::Kind::Local,
        }
    }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -

/// List and update installed crates
#[derive(Parser)]
#[command(name = "cargo", bin_name = "cargo", styles = CLAP_STYLING)]
enum Cli {
    /// List and update installed crates
    List(List),
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -

#[derive(Clone)]
enum OutputFormat {
    Markdown,
    Json,
    JsonPretty,
    Rust,
    RustPretty,
}

use OutputFormat::{Json, JsonPretty, Markdown, Rust, RustPretty};

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
// Structs

#[derive(clap::Args, Clone)]
#[allow(clippy::struct_excessive_bools)]
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

    /// Ignore version requirements to update pinned crate(s)
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

    /// Install crate(s)
    #[arg(long)]
    install: bool,

    /// Locked (only relevant to `--install`)
    #[arg(long)]
    locked: bool,

    /// Print readme
    #[arg(short, long)]
    readme: bool,

    /// List/update crates matching given pattern(s)
    #[arg(value_name = "PATTERN")]
    include: Vec<String>,
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -

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
// Functions

fn main() -> Result<()> {
    let Cli::List(cli) = Cli::parse();

    if cli.readme {
        #[cfg(unix)]
        Pager::with_pager("bat -pl md").setup();

        print!("{}", include_str!("../../README.md"));
        return Ok(());
    }

    if cli.install {
        println!("{}\n", "# Install".magenta().bold());

        let mut shell = Shell {
            dry_run: cli.dry_run,
            ..Default::default()
        };

        if cli.dry_run {
            shell.info = String::from("bash");
        }

        let mut locks = BTreeSet::new();

        for i in &cli.include {
            println!("{}\n", format!("## {i:?}").yellow().bold());

            let _ = shell.run(&[Command {
                command: if cli.locked {
                    locks.insert({
                        let mut name = i.as_str();

                        if name.contains('@') {
                            (name, _) = name.split_once('@').expect("split @");
                        }

                        if name.contains(' ') {
                            (name, _) = name.split_once(' ').expect("split space");
                        }

                        name.to_string()
                    });

                    format!("cargo install --locked {i}")
                } else {
                    format!("cargo install {i}")
                },
                ..Default::default()
            }]);
        }

        if cli.locked {
            let (mut installed, config_file) = load_config_file(&cli)?;
            installed.set_locks(&locks)?;
            installed.save(&config_file)?;
        }

        return Ok(());
    }

    inner(&cli)
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -

#[allow(clippy::too_many_lines)]
fn inner(cli: &List) -> Result<()> {
    let (installed, _config_file) = load_config_file(cli)?;

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
                }

                println!(
                    "{}\n",
                    "*No crates matching given pattern(s) are installed.*"
                        .yellow()
                        .italic()
                );
                std::process::exit(2);
            }

            let kinds = if cli.all_kinds {
                cargo_list::ALL_KINDS.to_vec()
            } else {
                cli.kind
                    .par_iter()
                    .map(Kind::into)
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
                                (String::new(), c.available.clone())
                            } else {
                                update_pinned += 1;
                                (pinned.clone(), c.newer.join(", "))
                            }
                        } else {
                            (String::new(), c.available.clone())
                        };

                        if cli.outdated_rust {
                            if c.outdated_rust {
                                // Include crate compiled with outdated Rust
                                t.push(Row::new(
                                    number.to_string().normal(),
                                    c.name.normal(),
                                    pinned.normal(),
                                    c.installed.green(),
                                    if pinned.is_empty() {
                                        "".normal()
                                    } else {
                                        available.bold().yellow()
                                    },
                                    c.rust_version.red(),
                                    cli.outdated_rust,
                                ));
                                number += 1;
                                outdated += 1;
                            } else if cli.ignore_req && !c.newer.is_empty() {
                                // Include outdated pinned
                                t.push(Row::new(
                                    number.to_string().normal(),
                                    c.name.normal(),
                                    pinned.normal(),
                                    c.installed.green(),
                                    if pinned.is_empty() {
                                        "".normal()
                                    } else {
                                        available.bold().yellow()
                                    },
                                    c.rust_version.green(),
                                    cli.outdated_rust,
                                ));
                                number += 1;
                            } else if !cli.outdated {
                                // Include every crate
                                t.push(Row::new(
                                    number.to_string().normal(),
                                    c.name.normal(),
                                    pinned.normal(),
                                    c.installed.green(),
                                    if pinned.is_empty() {
                                        "".normal()
                                    } else {
                                        available.bold().yellow()
                                    },
                                    c.rust_version.green(),
                                    cli.outdated_rust,
                                ));
                                number += 1;
                            }
                        } else {
                            // No -R...
                            if c.outdated {
                                // Include outdated crate
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
                            } else if cli.ignore_req && !c.newer.is_empty() {
                                // Include outdated pinned crate
                                t.push(Row::new(
                                    number.to_string().normal(),
                                    c.name.normal(),
                                    pinned.normal(),
                                    c.installed.yellow(),
                                    c.newer[0].bold().green(),
                                    "".normal(),
                                    cli.outdated_rust,
                                ));
                                number += 1;
                            } else if !cli.outdated {
                                // Include every crate
                                t.push(Row::new(
                                    number.to_string().normal(),
                                    c.name.normal(),
                                    pinned.normal(),
                                    c.installed.green(),
                                    if pinned.is_empty() {
                                        "".normal()
                                    } else {
                                        available.bold().yellow()
                                    },
                                    "".normal(),
                                    cli.outdated_rust,
                                ));
                                number += 1;
                            }
                        }
                    } else if c.kind == cargo_list::Kind::Git || !cli.outdated {
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

                    if update_pinned > 0 {
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
                            let _ = shell.run(&[Command {
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
                        c.include = updates.keys().map(ToString::to_string).collect();
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
                            let _ = shell.run(&[Command {
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

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -

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

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -

fn load_config_file(cli: &List) -> Result<(Crates, PathBuf)> {
    let mut sp = Spinner::new(Spinners::Line, String::new());

    let mut installed = Crates::from_include(
        &get_config_path(&cli.config),
        &cli.include.iter().map(String::as_str).collect::<Vec<_>>(),
    )?;

    let config_dir = dirs::config_dir().unwrap().join(env!("CARGO_PKG_NAME"));
    let config_file = config_dir.join("config.toml");

    if !config_dir.exists() {
        if let Err(e) = std::fs::create_dir_all(&config_dir) {
            sp.stop();
            eprint!("\x1b[2K\r");
            return Err(anyhow!(
                "Failed to create the configuration directory `{}`: `{e}`!",
                config_dir.display(),
            ));
        }
    } else if !config_dir.is_dir() {
        sp.stop();
        eprint!("\x1b[2K\r");
        return Err(anyhow!(
            "Configuration directory `{}` path exists but is not a directory!",
            config_dir.display(),
        ));
    }

    if config_file.exists() {
        if config_file.is_file() {
            match std::fs::read_to_string(&config_file) {
                Ok(s) => match toml::from_str::<BTreeMap<String, CrateOptions>>(&s) {
                    Ok(config) => {
                        installed.set_locked(&config);
                        sp.stop();
                        eprint!("\x1b[2K\r");

                        Ok((installed, config_file))
                    }
                    Err(e) => {
                        sp.stop();
                        eprint!("\x1b[2K\r");
                        Err(anyhow!(
                            "Configuration file `{}` is not valid TOML format: `{e}`!",
                            config_file.display(),
                        ))
                    }
                },
                Err(e) => {
                    sp.stop();
                    eprint!("\x1b[2K\r");
                    Err(anyhow!("Could not read configuration file: `{e}`!"))
                }
            }
        } else {
            sp.stop();
            eprint!("\x1b[2K\r");
            Err(anyhow!(
                "Configuration file `{}` path exists but is not a file!",
                config_file.display(),
            ))
        }
    } else {
        // Create a configuration file
        if let Err(e) = installed.save(&config_file) {
            sp.stop();
            eprint!("\x1b[2K\r");
            Err(e)
        } else {
            load_config_file(cli)
        }
    }
}
