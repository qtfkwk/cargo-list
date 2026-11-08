//! CLI

//--------------------------------------------------------------------------------------------------
// Crates

use {
    anyhow::{Result, anyhow},
    cargo_list::*,
    clap::{Parser, ValueEnum},
    clap_cargo::style::CLAP_STYLING,
    indexmap::IndexSet,
    spinners::{Spinner, Spinners},
    sprint::Shell,
    std::collections::BTreeSet,
    veg::colored::{ColoredString, Colorize, Veg},
};

#[cfg(unix)]
use pager2::Pager;

//--------------------------------------------------------------------------------------------------
// Constants

const README: &str = include_str!("../README.md");

const TABLE_HEAD: &str = "#|Name|Pinned|Installed|Available|Locked\n-:|-|-|-|-|-\n\n";

//--------------------------------------------------------------------------------------------------
// Enums

/// List and update installed crates
#[derive(Parser)]
#[command(name = "cargo", bin_name = "cargo", styles = CLAP_STYLING)]
enum Command {
    /// List and update installed crates
    List(List),
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -

#[derive(Clone, ValueEnum)]
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
        match self {
            Markdown => "markdown",
            Json => "json",
            JsonPretty => "json-pretty",
            Rust => "rust",
            RustPretty => "rust-pretty",
        }
        .fmt(f)
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
        conflicts_with = "update",
    )]
    output_format: OutputFormat,

    /// Local crates
    #[arg(short = 'l', long)]
    local: bool,

    /// Git crates
    #[arg(short = 'g', long)]
    git: bool,

    /// External crates (default)
    #[arg(short = 'e', long)]
    external: bool,

    /// Local, git, and external crates
    #[arg(
        short = 'a',
        long,
        conflicts_with_all = &[
            "external",
            "git",
            "local",
        ],
    )]
    all: bool,

    /// Only show *outdated* crates
    #[arg(short = 'o', long)]
    outdated: bool,

    /// Ignore pinned version requirements
    #[arg(short = 'I', long)]
    ignore_req: bool,

    /// Consider a crate to be outdated if compiled with a Rust version
    /// different than the active toolchain
    #[arg(short = 'R', long)]
    outdated_rust: bool,

    /// Update outdated crates
    #[arg(short = 'u', long)]
    update: bool,

    /// Dry run (only relevant to `-u`)
    #[arg(short = 'n', long)]
    dry_run: bool,

    /// Install crate(s)
    #[arg(long)]
    install: bool,

    /// Locked (only relevant to `--install`)
    #[arg(long)]
    locked: bool,

    /// Print readme
    #[arg(short = 'r', long)]
    readme: bool,

    /// List/update crates matching given pattern(s)
    #[arg(value_name = "PATTERN")]
    include: Vec<String>,
}
// TODO: Add a -p PROTOCOL option
// - Force using a particular protocol for this run
// - Becomes the future default if the config file doesn't exist

impl List {
    fn run(&self) -> Result<()> {
        if self.readme {
            self.print_readme()
        } else if self.install {
            self.install_crates()
        } else if self.update {
            self.update_crates()
        } else {
            self.list_crates()
        }
    }

    #[allow(clippy::unused_self)]
    #[allow(clippy::unnecessary_wraps)]
    fn print_readme(&self) -> Result<()> {
        page_output("md");
        print!("{README}");
        Ok(())
    }

    fn list_crates(&self) -> Result<()> {
        let (crates, kinds) = self.get_crates()?;

        match self.output_format {
            Markdown => {
                if crates.is_empty() {
                    if self.include.is_empty() {
                        Err(anyhow!(ListError::new(
                            1,
                            "*No crates are installed.*".into(),
                        )))
                    } else {
                        Err(anyhow!(ListError::new(
                            2,
                            format!("*No crates matching `{:?}` are installed.*", self.include),
                        )))
                    }
                } else {
                    for kind in &kinds {
                        println!("{}\n", format!("# {kind:?}").magenta().bold());

                        let mut t = if self.outdated_rust {
                            Veg::table(&TABLE_HEAD.replace("\n\n", "|-").replace("\n-", "|Rust\n-"))
                        } else {
                            Veg::table(TABLE_HEAD.trim())
                        };

                        let mut outdated = 0;
                        let mut outdated_pinned = 0;
                        for (i, c) in crates.kind(*kind).into_iter().enumerate() {
                            self.row(i, c, &mut t, &mut outdated, &mut outdated_pinned);
                        }

                        // Print the table
                        if !t.is_empty() {
                            println!("{}", t.markdown()?);
                        }

                        // Print a summary
                        if *kind == External {
                            if outdated == 0 {
                                println!(
                                    "{}\n",
                                    format!(
                                        "**All {}external crates are up-to-date!**",
                                        if outdated_pinned > 0 { "unpinned " } else { "" },
                                    )
                                    .green()
                                    .bold(),
                                );
                            } else {
                                println!(
                                    "{}\n",
                                    format!(
                                        "**Need to update {} external crate{}!**",
                                        outdated,
                                        s(outdated),
                                    )
                                    .red()
                                    .bold(),
                                );
                            }

                            if outdated_pinned > 0 {
                                println!(
                                    "{}\n",
                                    format!(
                                        "*Consider updating {} pinned external crate{}{}.*",
                                        outdated_pinned,
                                        s(outdated_pinned),
                                        if self.ignore_req {
                                            ""
                                        } else {
                                            "... add `-I` to view"
                                        },
                                    )
                                    .yellow()
                                    .italic(),
                                );
                            }
                        }
                    }

                    Ok(())
                }
            }
            Json => {
                page_output("json");
                println!("{}", serde_json::to_string(&crates)?);
                Ok(())
            }
            JsonPretty => {
                page_output("json");
                println!("{}", serde_json::to_string_pretty(&crates)?);
                Ok(())
            }
            Rust => {
                page_output("rust");
                println!("{crates:?}");
                Ok(())
            }
            RustPretty => {
                page_output("rust");
                println!("{crates:#?}");
                Ok(())
            }
        }
    }

    fn update_crates(&self) -> Result<()> {
        let (crates, kinds) = self.get_crates()?;

        if crates.outdated(self.ignore_req).is_empty() {
            self.list_crates()
        } else {
            self.list_crates()?;

            for kind in &kinds {
                if *kind != External {
                    continue;
                }

                let mut updates = vec![];

                for c in crates.kind(*kind) {
                    if self.outdated_rust && c.outdated_rust {
                        // Update crate installed/compiled with outdated Rust
                        updates.push(c);
                    } else if c.outdated {
                        // Update outdated crate
                        updates.push(c);
                    } else if self.ignore_req && !c.newer.is_empty() {
                        // Update outdated pinned crate
                        updates.push(c);
                    }
                }

                if !updates.is_empty() {
                    println!("{}\n", format!("# {kind:?}").magenta().bold());

                    let mut shell = Shell {
                        dry_run: self.dry_run,
                        ..Default::default()
                    };

                    if self.dry_run {
                        shell.info = "bash".into();
                    }

                    for c in &updates {
                        println!("{}\n", format!("## {}", c.name).yellow().bold());

                        let _ = shell.run(&[sprint::Command::new(
                            &c.update_command(self.ignore_req).join(" "),
                        )]);
                        // TODO: Handle failed update
                    }
                }
            }

            self.list_crates()
        }
    }

    fn install_crates(&self) -> Result<()> {
        println!("{}\n", "# Install".magenta().bold());

        let mut shell = Shell {
            dry_run: self.dry_run,
            ..Default::default()
        };

        if self.dry_run {
            shell.info = String::from("bash");
        }

        let mut crate_names = BTreeSet::new();

        for i in &self.include {
            println!("{}\n", format!("## {i:?}").yellow().bold());

            let _ = shell.run(&[sprint::Command {
                command: if self.locked {
                    crate_names.insert({
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
        // TODO: Handle failed install

        if self.locked {
            self.get_crates()?.0.lock(&crate_names)
        } else {
            Ok(())
        }
    }

    fn get_crates(&self) -> Result<(Crates, IndexSet<Kind>)> {
        spinner("", || {
            let kinds = self.kinds();

            match Crates::new(&to_strs(&self.include), &kinds) {
                Ok(crates) => Ok((crates, kinds)),
                Err(e) => Err(e),
            }
        })
    }

    fn kinds(&self) -> IndexSet<Kind> {
        if self.all {
            Kind::all()
        } else {
            let mut kinds = [
                (self.local, Local),
                (self.git, Git),
                (self.external, External),
            ]
            .into_iter()
            .filter_map(|(cond, kind)| cond.then_some(kind))
            .collect::<IndexSet<_>>();

            if kinds.is_empty() {
                kinds.insert(External);
            }

            kinds
        }
    }

    #[allow(clippy::too_many_lines)]
    fn row(
        &self,
        i: usize,
        c: &Crate,
        t: &mut Veg,
        outdated: &mut usize,
        outdated_pinned: &mut usize,
    ) {
        if c.kind == External {
            let available = if c.pinned.is_some() && !c.newer.is_empty() {
                // Pinned AND outdated

                *outdated_pinned += 1;

                c.newer
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            } else {
                // Not pinned OR pinned and up-to-date

                c.available
                    .as_ref()
                    .map_or_else(String::new, ToString::to_string)
            };

            if self.outdated_rust {
                // -R

                if c.outdated_rust {
                    // Crate is installed/compiled with outdated Rust

                    *outdated += 1;

                    t.push(Row::new(
                        number(i),
                        c.name.normal(),
                        opt_normal(c.pinned.as_ref()),
                        c.installed.to_string().green(),
                        opt_warn(c.pinned.as_ref(), &available),
                        locked(c.locked),
                        c.rust_version.to_string().red(),
                        self.outdated_rust,
                    ));
                } else if !c.newer.is_empty() {
                    // Crate is pinned but there are updates

                    t.push(Row::new(
                        number(i),
                        c.name.normal(),
                        opt_normal(c.pinned.as_ref()),
                        c.installed.to_string().green(),
                        opt_warn(c.pinned.as_ref(), &available),
                        locked(c.locked),
                        c.rust_version.to_string().green(),
                        self.outdated_rust,
                    ));
                } else if !self.outdated {
                    // Include up-to-date crates

                    t.push(Row::new(
                        number(i),
                        c.name.normal(),
                        opt_normal(c.pinned.as_ref()),
                        c.installed.to_string().green(),
                        empty(),
                        locked(c.locked),
                        c.rust_version.to_string().green(),
                        self.outdated_rust,
                    ));
                }
            } else {
                // No -R...

                if c.outdated {
                    // Include outdated crate

                    *outdated += 1;

                    t.push(Row::new(
                        number(i),
                        c.name.normal(),
                        opt_normal(c.pinned.as_ref()),
                        c.installed.to_string().red(),
                        available.bold(),
                        locked(c.locked),
                        red_or_green(c.outdated_rust, &c.rust_version),
                        self.outdated_rust,
                    ));
                } else if !c.newer.is_empty() {
                    // Include outdated pinned crate

                    t.push(Row::new(
                        number(i),
                        c.name.normal(),
                        opt_normal(c.pinned.as_ref()),
                        c.installed.to_string().yellow(),
                        c.newer[0].to_string().bold().green(),
                        locked(c.locked),
                        empty(),
                        self.outdated_rust,
                    ));
                } else if !self.outdated {
                    // Include every crate

                    t.push(Row::new(
                        number(i),
                        c.name.normal(),
                        opt_normal(c.pinned.as_ref()),
                        c.installed.to_string().green(),
                        empty(),
                        locked(c.locked),
                        empty(),
                        self.outdated_rust,
                    ));
                }
            }
        } else if !self.outdated {
            // Include git and local crates

            if self.outdated_rust {
                // -R

                if c.outdated_rust {
                    t.push(Row::new(
                        number(i),
                        c.name.normal(),
                        empty(),
                        c.installed.to_string().cyan(),
                        empty(),
                        locked(c.locked),
                        c.rust_version.to_string().red(),
                        self.outdated_rust,
                    ));
                } else {
                    t.push(Row::new(
                        number(i),
                        c.name.normal(),
                        empty(),
                        c.installed.to_string().cyan(),
                        empty(),
                        locked(c.locked),
                        c.rust_version.to_string().green(),
                        self.outdated_rust,
                    ));
                }
            } else {
                // No -R...

                t.push(Row::new(
                    number(i),
                    c.name.normal(),
                    empty(),
                    c.installed.to_string().cyan(),
                    empty(),
                    locked(c.locked),
                    empty(),
                    self.outdated_rust,
                ));
            }
        }
    }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
struct ListError {
    pub exit_code: i32,
    pub message: String,
}

impl ListError {
    fn new(exit_code: i32, message: String) -> ListError {
        ListError { exit_code, message }
    }
}

//--------------------------------------------------------------------------------------------------
// Functions

fn main() {
    match Command::parse() {
        Command::List(list) => {
            if let Err(e) = list.run() {
                eprintln!("{}", format!("Error: {e}").red().bold());
                std::process::exit(e.downcast_ref::<ListError>().map_or(1, |e| e.exit_code));
            }
        }
    }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -

fn to_strs(v: &[String]) -> Vec<&str> {
    v.iter().map(String::as_str).collect::<Vec<_>>()
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -

fn spinner<F, T>(msg: &str, f: F) -> T
where
    F: FnOnce() -> T,
{
    let mut sp = Spinner::new(Spinners::Line, msg.into());
    let r = f();
    sp.stop();
    eprint!("\x1b[2K\r");
    r
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -

fn page_output(lang: &str) {
    #[cfg(unix)]
    Pager::with_pager(&format!("bat -pl {lang}")).setup();
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -

fn s(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -

fn locked(cond: bool) -> ColoredString {
    if cond { "✔".bold().green() } else { empty() }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -

fn number(i: usize) -> ColoredString {
    (i + 1).to_string().normal()
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -

fn opt_normal<T: std::fmt::Display>(v: Option<&T>) -> ColoredString {
    v.map_or_else(empty, |x| x.to_string().normal())
}

fn opt_warn<T: std::fmt::Display>(o: Option<&T>, s: &str) -> ColoredString {
    if o.is_some() {
        s.yellow().bold()
    } else {
        empty()
    }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -

fn red_or_green(cond: bool, v: &semver::Version) -> ColoredString {
    if cond {
        v.to_string().red()
    } else {
        v.to_string().green()
    }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -

fn empty() -> ColoredString {
    "".normal()
}
