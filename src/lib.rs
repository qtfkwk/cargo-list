#![doc = include_str!("../t/LIBRARY.md")]

//--------------------------------------------------------------------------------------------------
// Crates

use {
    anyhow::{Result, anyhow},
    clap::{Parser, ValueEnum},
    clap_cargo::style::CLAP_STYLING,
    dirs::home_dir,
    indexmap::IndexSet,
    rayon::prelude::*,
    regex::RegexSet,
    reqwest::blocking::Client,
    serde::{Deserialize, Serialize},
    spinners::{Spinner, Spinners},
    sprint::{Pipe, Shell},
    std::{
        collections::{BTreeMap, BTreeSet},
        fmt::Write,
        fs::File,
        path::PathBuf,
        sync::LazyLock,
    },
    veg::colored::{ColoredString, Colorize, Veg},
};

#[cfg(unix)]
use pager2::Pager;

//--------------------------------------------------------------------------------------------------
// Constants

const README: &str = include_str!("../README.md");

const TABLE_HEAD: &str = "#|Name|Pinned|Installed|Available|Locked\n-:|-|-|-|-|-\n\n";

//--------------------------------------------------------------------------------------------------
// Statics

static CLIENT: LazyLock<Client> = LazyLock::new(|| {
    let mut client = Client::builder().user_agent("cargo-list");

    // If the CARGO_LIST_PROXY environment variable is set,
    // try to use its value to set the reqwest client proxy
    if let Ok(url) = std::env::var("CARGO_LIST_PROXY") {
        match reqwest::Proxy::all(&url) {
            Ok(proxy) => {
                client = client.proxy(proxy);
            }
            Err(e) => {
                eprintln!(
                    "\
                        Warning: The `CARGO_LIST_PROXY` environment variable is set but is invalid \
                        ({e}); ignoring!\
                    ",
                );
            }
        }
    }

    client.build().expect("create reqwest client")
});

/// Configuration file
pub static CONFIG_FILE: LazyLock<PathBuf> = LazyLock::new(|| {
    dirs::config_dir()
        .unwrap()
        .join(env!("CARGO_PKG_NAME"))
        .join("config.toml")
});

//--------------------------------------------------------------------------------------------------
// Structs

/// Crates
#[derive(Debug, Serialize)]
pub struct Crates {
    protocol: Protocol,
    installed: BTreeMap<String, Crate>,
    rust: semver::Version,
}

impl Crates {
    /**
    Get the current state of installed crates

    - `include`: Slice of regex patterns to filter crates by name;
      empty slice `&[]` means "all crates"
    - `kinds`: Filter crates by kind; empty kinds is an error
    - `protocol`: Method used to fetch versions from Crates.io;
      if `None` it uses the protocol set in the config file, or the default if not set
    */
    pub fn new(
        include: &[&str],
        kinds: &IndexSet<Kind>,
        protocol: Option<Protocol>,
    ) -> Result<Crates> {
        if kinds.is_empty() {
            return Err(anyhow!("No kinds"));
        }

        let crates = CratesJson::new()?;
        let config = Config::new(&crates, protocol)?;
        let protocol = config.protocol;
        let rust = get_rust_version()?;
        let installed = crates.to_map(include, kinds, &rust, &config)?;

        let crates = Crates {
            protocol,
            installed,
            rust,
        };

        crates.save_config()?;

        Ok(crates)
    }

    /// Are any crates installed?
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.installed.is_empty()
    }

    /// Set crates to be "locked"
    pub fn lock(&mut self, crate_names: &BTreeSet<String>) -> Result<()> {
        for name in crate_names {
            self.installed
                .get_mut(name)
                .ok_or_else(|| anyhow!("Invalid crate name `{name}`"))?
                .locked = true;
        }

        self.save_config()
    }

    /// Save the config file
    pub fn save_config(&self) -> Result<()> {
        Config {
            protocol: self.protocol,
            crates: self
                .installed
                .iter()
                .map(|(k, v)| (k.clone(), CrateConfig { locked: v.locked }))
                .collect(),
        }
        .save()
    }

    /// Return crate references for crates of the given kind
    #[must_use]
    pub fn kind(&self, kind: Kind) -> Vec<&Crate> {
        self.installed.values().filter(|x| x.kind == kind).collect()
    }

    /// Installed crates
    #[must_use]
    pub fn installed(&self) -> &BTreeMap<String, Crate> {
        &self.installed
    }

    /// Outdated crates
    #[must_use]
    pub fn outdated(&self, include_pinned: bool) -> BTreeMap<&String, &Crate> {
        if include_pinned {
            self.installed
                .par_iter()
                .filter_map(|(name, c)| {
                    (c.outdated || (c.pinned.is_some() && !c.newer.is_empty())).then_some((name, c))
                })
                .collect()
        } else {
            self.installed
                .par_iter()
                .filter_map(|(name, c)| c.outdated.then_some((name, c)))
                .collect()
        }
    }

    /// Protocol
    #[must_use]
    pub fn protocol(&self) -> &Protocol {
        &self.protocol
    }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -

/// Individual installed crate
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, Serialize)]
pub struct Crate {
    /// Name
    pub name: String,

    /// Kind
    pub kind: Kind,

    /// Pinned
    pub pinned: Option<semver::VersionReq>,

    /// Installed
    pub installed: semver::Version,

    /// Available
    pub available: Option<semver::Version>,

    /// Newer
    pub newer: Vec<semver::Version>,

    /// Rust version
    pub rust_version: semver::Version,

    /// Outdated
    pub outdated: bool,

    /// Outdated rust
    pub outdated_rust: bool,

    /// Locked
    pub locked: bool,

    prerelease: bool,
    source: String,
    json: CrateJson,
}

impl Crate {
    /// Generate the cargo install command to update the crate
    pub fn update_command(&self, pinned: bool) -> Vec<String> {
        let mut r = vec!["cargo", "install"];

        if self.json.no_default_features {
            r.push("--no-default-features");
        }

        let features = (!self.json.features.is_empty()).then_some(self.json.features.join(","));
        if let Some(features) = &features {
            r.push("-F");
            r.push(features);
        }

        let pinned_version = self.pinned.as_ref().map(ToString::to_string);
        if !pinned && let Some(version) = &pinned_version {
            r.push("--version");
            r.push(version);
        }

        r.push("--profile");
        r.push(&self.json.profile);

        r.push("--target");
        r.push(&self.json.target);

        if self.outdated_rust {
            r.push("--force");
        }

        if self.kind == Git {
            r.push("--git");
            r.push(&self.source[4..self.source.find('#').unwrap_or(self.source.len())]);
            for bin in &self.json.bins {
                r.push(bin);
            }
        } else {
            r.push(&self.name);
        }

        if self.locked {
            r.push("--locked");
        }

        r.into_iter().map(String::from).collect()
    }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -

/// Deserialized `~/.cargo/.crates2.json`
#[derive(Debug, Serialize, Deserialize)]
pub struct CratesJson {
    installs: BTreeMap<String, CrateJson>,
}

impl CratesJson {
    /// Deserialize the `~/.cargo/.crates2.json` file
    pub fn new() -> Result<CratesJson> {
        let home = home_dir().ok_or_else(|| anyhow!("Failed to get the home directory"))?;
        let crates2_json = home.join(".cargo").join(".crates2.json");
        let file = File::open(&crates2_json)
            .map_err(|e| anyhow!("Failed to open `{}`: {e}", crates2_json.display()))?;
        let r: CratesJson = serde_json::from_reader(file)
            .map_err(|e| anyhow!("Failed to parse `{}`: {e}", crates2_json.display()))?;
        Ok(r)
    }

    /// Convert each [`CrateJson`] to a [`Crate`] and index by its name
    pub fn to_map(
        self,
        include: &[&str],
        kinds: &IndexSet<Kind>,
        rust_version: &semver::Version,
        config: &Config,
    ) -> Result<BTreeMap<String, Crate>> {
        if kinds.is_empty() {
            return Err(anyhow!("No kinds"));
        }

        let include_re = (!include.is_empty())
            .then(|| RegexSet::new(include))
            .transpose()?;

        let results = self
            .installs
            .into_par_iter()
            .map(|(k, v)| {
                let name = k
                    .split_once(' ')
                    .map(|(name, _)| name)
                    .ok_or_else(|| anyhow!("{k}"))?;

                if include_re.as_ref().is_some_and(|re| !re.is_match(name)) {
                    return Ok(None);
                }

                v.into_crate(&k, rust_version, config)
                    .map(|c| kinds.contains(&c.kind).then_some((c.name.clone(), c)))
            })
            .collect::<Vec<Result<Option<(String, Crate)>>>>();

        let mut crates = BTreeMap::new();
        let mut errors = Vec::new();
        for result in results {
            match result {
                Ok(Some((name, krate))) => {
                    crates.insert(name, krate);
                }
                Ok(None) => {}
                Err(error) => errors.push(error),
            }
        }

        if errors.is_empty() {
            Ok(crates)
        } else {
            Err(anyhow!(CargoListError::new(
                3,
                format!(
                    "Failed to process {} installs keys:\n\n{}",
                    errors.len(),
                    errors
                        .iter()
                        .enumerate()
                        .fold(String::new(), |mut s, (i, k)| {
                            let _ = writeln!(s, "{}. `{k}`", i + 1);
                            s
                        }),
                ),
            )))
        }
    }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -

/// Deserialized `~/.cargo/.crates2.json` `installed` value
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CrateJson {
    version_req: Option<semver::VersionReq>,
    bins: Vec<String>,
    features: Vec<String>,
    all_features: bool,
    no_default_features: bool,
    profile: String,
    target: String,
    rustc: Option<String>,
}

impl CrateJson {
    fn into_crate(
        self,
        k: &str,
        available_rust_version: &semver::Version,
        config: &Config,
    ) -> Result<Crate> {
        // Parse from the `CratesJson.installed` key
        let mut s = k.split(' ');
        let name = s.next().ok_or_else(|| anyhow!("No name"))?.to_string();
        let installed = parse_to_version(s.next().ok_or_else(|| anyhow!("No installed"))?)?;
        let prerelease = !installed.pre.is_empty();
        let source = s
            .next()
            .ok_or_else(|| anyhow!("No source"))?
            .strip_prefix('(')
            .ok_or_else(|| anyhow!("No `(` prefix"))?
            .strip_suffix(')')
            .ok_or_else(|| anyhow!("No `)` suffix"))?
            .to_string();

        // Get the kind
        let kind = Kind::from(&source);

        // Get the pinned version requirement
        let pinned = self.version_req.clone();

        // Determine if the crate is outdated
        let (available, newer, outdated) = match kind {
            External => {
                let (available, newer) =
                    config
                        .protocol
                        .get_latest(&name, self.version_req.as_ref(), prerelease)?;

                let outdated = installed < available;

                (Some(available), newer, outdated)
            }
            Git => {
                // TODO: Check if upstream has newer releases/commits
                (None, vec![], false)
            }
            Local => {
                // TODO: Check if the local path `Cargo.toml` has a newer version
                (None, vec![], false)
            }
        };

        // Rust version the crate was compiled/installed with and whether outdated
        let rust_version = if let Some(s) = &self.rustc {
            parse_to_version(
                s.strip_prefix("rustc ")
                    .ok_or_else(|| anyhow!("No `rustc ` prefix"))?
                    .split_once(' ')
                    .ok_or_else(|| anyhow!("No ` ` split"))?
                    .0,
            )?
        } else {
            return Err(anyhow!("No rustc"));
        };
        let outdated_rust = rust_version < *available_rust_version;

        // Get the crate's locked preference from the config
        let locked = config.is_locked(&name);

        Ok(Crate {
            name,
            kind,
            pinned,
            installed,
            available,
            newer,
            rust_version,
            outdated,
            outdated_rust,
            locked,
            prerelease,
            source,
            json: self,
        })
    }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -

/// Configuration
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Config {
    protocol: Protocol,
    crates: BTreeMap<String, CrateConfig>,
}

impl Config {
    fn new(crates_json: &CratesJson, protocol: Option<Protocol>) -> Result<Config> {
        if CONFIG_FILE.exists() {
            if CONFIG_FILE.is_file() {
                // Deserialize the config file

                let config_str = std::fs::read_to_string(&*CONFIG_FILE).map_err(|e| {
                    anyhow!(
                        "Failed to read config file `{}`: {e}",
                        CONFIG_FILE.display(),
                    )
                })?;

                let mut config: Config = toml::from_str(&config_str).map_err(|e| {
                    anyhow!(
                        "Failed to deserialize config file `{}`: {e}",
                        CONFIG_FILE.display(),
                    )
                })?;

                if let Some(protocol) = protocol {
                    config.protocol = protocol;
                }

                Ok(config)
            } else {
                Err(anyhow!(
                    "Config file `{}` exists but is not a file",
                    CONFIG_FILE.display(),
                ))
            }
        } else {
            // Create a default config file

            let mut config = Config::default();

            if let Some(protocol) = protocol {
                config.protocol = protocol;
            }

            let results = crates_json
                .installs
                .par_iter()
                .map(|(k, _v)| {
                    let name = k
                        .split_once(' ')
                        .map(|(name, _)| name)
                        .ok_or_else(|| anyhow!("{k}"))?
                        .to_owned();

                    Ok(Some((name, CrateConfig { locked: false })))
                })
                .collect::<Vec<Result<Option<(String, CrateConfig)>>>>();

            let mut crates = BTreeMap::new();
            let mut errors = Vec::new();
            for result in results {
                match result {
                    Ok(Some((name, crate_config))) => {
                        crates.insert(name, crate_config);
                    }
                    Ok(None) => {}
                    Err(error) => errors.push(error),
                }
            }

            if errors.is_empty() {
                config.crates = crates;
                config.save()?;
                Ok(config)
            } else {
                Err(anyhow!(
                    "Failed to process {} installs keys:\n\n{}",
                    errors.len(),
                    errors
                        .iter()
                        .enumerate()
                        .fold(String::new(), |mut s, (i, k)| {
                            let _ = writeln!(s, "{}. `{k}`", i + 1);
                            s
                        }),
                ))
            }
        }
    }

    fn is_locked(&self, name: &str) -> bool {
        self.crates.get(name).is_some_and(|x| x.locked)
    }

    fn save(&self) -> Result<()> {
        let config_dir = CONFIG_FILE
            .parent()
            .ok_or_else(|| anyhow!("Failed to get directory for `{}`", CONFIG_FILE.display()))?;

        std::fs::create_dir_all(config_dir)
            .map_err(|e| anyhow!("Failed to create directory `{}`: {e}", config_dir.display()))?;

        let config_toml = format!(
            "\
# Configuration file for `cargo-list`\n\
#\n\
# This file is automatically updated by `cargo list`.\n\
#\n\
# It may also be edited to the user's desired configuration.\n\
#\n\
# Reasons to edit:\n\
#\n\
# 1. Change the protocol:\n\
#\n\
# protocol = \"sparse\" # default\n\
# protocol = \"rest-api\" # legacy\n\
#\n\
# 2. Toggle `locked` for one or more crates:\n\
#\n\
# [crates.crate-name]\n\
# locked = false # default\n\
# locked = true # add `--locked` to `cargo install` command\n\
#\n\
# Alternatively, use `cargo list --install --locked ...` to install crates and
# `locked` will be set to `true`.\n\
\n\
{}\n\
            ",
            toml::to_string_pretty(self).map_err(|e| anyhow!("Failed to serialize config: {e}"))?,
        );

        std::fs::write(&*CONFIG_FILE, &config_toml)
            .map_err(|e| anyhow!("Failed to write to `{}`: {e}", CONFIG_FILE.display()))?;

        Ok(())
    }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -

/// Crate configuration
#[derive(Debug, Deserialize, Serialize)]
pub struct CrateConfig {
    locked: bool,
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -

/**
Deserialize the crate versions object returned via the crates.io API
(`https://crates.io/api/v1/crates/{name}/versions`) in the [`latest()`] function
*/
#[derive(Debug, Deserialize)]
struct ApiVersionsJson {
    versions: Vec<ApiVersionJson>,
}

impl ApiVersionsJson {
    fn iter(&self) -> std::slice::Iter<'_, ApiVersionJson> {
        self.versions.iter()
    }

    fn available(&self, prerelease: bool) -> Vec<&ApiVersionJson> {
        self.iter().filter(|x| x.is_available(prerelease)).collect()
    }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -

#[derive(Debug, Deserialize)]
struct ApiVersionJson {
    num: semver::Version,

    #[serde(default)]
    yanked: bool,
}

impl ApiVersionJson {
    fn is_available(&self, prerelease: bool) -> bool {
        if !prerelease && !self.num.pre.is_empty() {
            return false;
        }

        !self.yanked
    }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -

#[derive(Debug, Deserialize)]
struct SparseVersionJson {
    vers: semver::Version,

    #[serde(default)]
    yanked: bool,
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -

/// Crate list table row
#[derive(Debug)]
pub struct Row {
    number: ColoredString,
    name: ColoredString,
    pinned: ColoredString,
    installed: ColoredString,
    available: ColoredString,
    locked: ColoredString,
    rust: ColoredString,
    outdated_rust: bool,
}

impl Row {
    /// Create a new row
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        number: ColoredString,
        name: ColoredString,
        pinned: ColoredString,
        installed: ColoredString,
        available: ColoredString,
        locked: ColoredString,
        rust: ColoredString,
        outdated_rust: bool,
    ) -> Box<Row> {
        Box::new(Row {
            number,
            name,
            pinned,
            installed,
            available,
            locked,
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
            self.locked.clone(),
        ];

        if self.outdated_rust {
            r.push(self.rust.clone());
        }

        r
    }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -

/// Custom error with an exit code and message
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct CargoListError {
    /// Exit code
    pub exit_code: i32,

    /// Message
    pub message: String,
}

impl CargoListError {
    /// Create a [`CargoListError`]
    #[must_use]
    pub fn new(exit_code: i32, message: String) -> CargoListError {
        CargoListError { exit_code, message }
    }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -

/// List command/args
#[derive(clap::Args, Clone)]
#[allow(clippy::struct_excessive_bools)]
#[command(about, version, max_term_width = 80)]
pub struct List {
    /// Include local crates
    #[arg(short = 'l', long)]
    local: bool,

    /// Include git crates
    #[arg(short = 'g', long)]
    git: bool,

    /// Include external crates (default)
    #[arg(short = 'e', long)]
    external: bool,

    /// Include local, git, and external crates; same as `-lge`
    #[arg(
        short = 'a',
        long,
        conflicts_with_all = &["local", "git", "external"],
    )]
    all: bool,

    /// Only show *outdated* crates / hide up-to-date crates
    #[arg(short = 'o', long)]
    outdated: bool,

    /// Protocol
    #[arg(short = 'p', long, value_name = "P", default_value_t)]
    protocol: Protocol,

    /// Ignore pinned version requirements when deciding if a crate is *outdated* for the purpose of
    /// listing or updating the crate
    #[arg(short = 'I', long)]
    ignore_req: bool,

    /// Consider a crate to be outdated if compiled with a Rust version different than the active
    /// toolchain
    #[arg(short = 'R', long)]
    outdated_rust: bool,

    /// Update outdated crates
    #[arg(short = 'u', long)]
    update: bool,

    /// Dry run (only relevant to `-u` / `--update`)
    #[arg(short = 'n', long)]
    dry_run: bool,

    /// Output format
    #[arg(
        short = 'f',
        long,
        value_name = "F",
        default_value_t,
        conflicts_with_all = &["update", "install"],
    )]
    format: OutputFormat,

    /// Install one or more crate(s)
    ///
    /// Same as `cargo install ...` except when combined with `--locked` which sets the locked
    /// status in the configuration file
    #[arg(long)]
    install: bool,

    /// Locked (only relevant to `--install`)
    #[arg(long)]
    locked: bool,

    /// Print readme
    #[arg(short = 'r', long)]
    readme: bool,

    /// Show running configuration and exit
    #[arg(short = 'c', long)]
    config: bool,

    /// List/update installed crates matching given pattern(s);
    /// if `--install` use the actual crate name
    #[arg(value_name = "PATTERN")]
    include: Vec<String>,
}

impl List {
    /// Run the list command
    pub fn run(&mut self) -> Result<()> {
        if self.readme {
            self.print_readme()
        } else if self.config {
            self.show_config()
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

    #[allow(clippy::unused_self)]
    #[allow(clippy::unnecessary_wraps)]
    fn show_config(&self) -> Result<()> {
        page_output("md");
        println!("# Configuration\n");
        let (crates, kinds) = self.get_crates()?;
        println!("- Configuration file: `{}`", CONFIG_FILE.display());
        println!("- Protocol: `{:?}`", crates.protocol());
        println!("- Kinds: `{kinds:?}`");
        println!();
        Ok(())
    }

    fn list_crates(&self) -> Result<()> {
        let (crates, kinds) = self.get_crates()?;

        match self.format {
            Markdown => {
                if crates.is_empty() {
                    if self.include.is_empty() {
                        Err(anyhow!(CargoListError::new(
                            1,
                            "*No crates are installed.*".into(),
                        )))
                    } else {
                        Err(anyhow!(CargoListError::new(
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

    fn install_crates(&mut self) -> Result<()> {
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

        self.include.clear();
        let (mut crates, _kinds) = self.get_crates()?;

        if self.locked {
            crates.lock(&crate_names)
        } else {
            Ok(())
        }
    }

    fn get_crates(&self) -> Result<(Crates, IndexSet<Kind>)> {
        spinner("", || {
            let kinds = self.kinds();

            match Crates::new(&to_strs(&self.include), &kinds, Some(self.protocol)) {
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

//--------------------------------------------------------------------------------------------------
// Enums

/// Crate kind
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, ValueEnum)]
pub enum Kind {
    /// Installed from a local directory
    Local,

    /// Installed from a git repository
    Git,

    /// Installed from <https://crates.io>
    #[default]
    External,
}

pub use Kind::{External, Git, Local};

impl Kind {
    /// All kinds
    #[must_use]
    pub fn all() -> IndexSet<Kind> {
        IndexSet::from([Local, Git, External])
    }

    fn from(source: &str) -> Kind {
        if source.starts_with("path+") {
            Local
        } else if source.starts_with("git+") {
            Git
        } else {
            External
        }
    }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -

/// Protocol used to query the Crates.io index
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    /// <https://doc.rust-lang.org/cargo/reference/registry-index.html>
    #[default]
    Sparse,

    /// <https://doc.rust-lang.org/cargo/reference/registry-web-api.html>
    RestApi,
}

pub use Protocol::*;

impl Protocol {
    fn get_latest(
        self,
        name: &str,
        version_req: Option<&semver::VersionReq>,
        prerelease: bool,
    ) -> Result<(semver::Version, Vec<semver::Version>)> {
        match self {
            Protocol::Sparse => sparse_latest(name, version_req, prerelease),
            Protocol::RestApi => rest_api_latest(name, version_req, prerelease),
        }
    }
}

impl std::fmt::Display for Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Sparse => "sparse",
            RestApi => "rest-api",
        }
        .fmt(f)
    }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -

/// List and update installed crates
#[derive(Parser)]
#[command(name = "cargo", bin_name = "cargo", styles = CLAP_STYLING)]
pub enum Command {
    /// List and update installed crates
    List(List),
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -

#[derive(Clone, Default, ValueEnum)]
enum OutputFormat {
    #[default]
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
// Functions

/**
Get the latest available (not prerelease or yanked) version(s) for a crate, optionally matching a
required version, via the Crates.io REST API
*/
pub fn rest_api_latest(
    name: &str,
    version_req: Option<&semver::VersionReq>,
    prerelease: bool,
) -> Result<(semver::Version, Vec<semver::Version>)> {
    let url = format!("https://crates.io/api/v1/crates/{name}/versions");
    let res = CLIENT.get(&url).send()?;
    let res = res.error_for_status()?;
    let versions = res.json::<ApiVersionsJson>()?;
    let available = versions.available(prerelease);

    if let Some(req) = version_req {
        let mut newer = vec![];
        for v in &available {
            if req.matches(&v.num) {
                return Ok((v.num.clone(), newer));
            }

            newer.push(v.num.clone());
        }

        // If we haven't found a match yet, but we are allowing prereleases,
        // it's possible the requirement string didn't explicitly opt-in to prereleases (like
        // `^2.0.0`) but the available versions are prereleases (like `2.0.0-rc.37`).
        // In this specific case, if we found *no* matching versions, we might want to be lenient,
        // but semver::VersionReq is strict.
        // However, if the user INSTALLED a prerelease, usually the version_req in `.crates2.json`
        // reflects that (e.g. it might be `=2.0.0-rc.37` or `^2.0.0-rc.37`).
        // If the error persists, it means even with prereleases included in `available`, none
        // matched `req`.
        Err(anyhow!(
            "\
                Failed to find an available version matching the requirement `{req}` \
                (available: {:?})\
            ",
            available
                .iter()
                .take(5)
                .map(|v| v.num.to_string())
                .collect::<Vec<_>>()
        ))
    } else if available.is_empty() {
        Err(anyhow!("Failed to find any available version"))
    } else {
        Ok((available[0].num.clone(), vec![]))
    }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -

/**
Get the latest available (not prerelease or yanked) version(s) for a crate, optionally matching a
required version, via the Crates.io sparse index
*/
pub fn sparse_latest(
    name: &str,
    version_req: Option<&semver::VersionReq>,
    prerelease: bool,
) -> Result<(semver::Version, Vec<semver::Version>)> {
    let name = name.to_ascii_lowercase();

    let path = match name.len() {
        1 => format!("1/{name}"),
        2 => format!("2/{name}"),
        3 => format!("3/{}/{name}", &name[..1]),
        _ => format!("{}/{}/{name}", &name[..2], &name[2..4]),
    };

    let url = format!("https://index.crates.io/{path}");
    let res = CLIENT.get(&url).send()?;
    let res = res.error_for_status()?;
    let body = res.text()?;

    let mut available = body
        .lines()
        .filter(|line| !line.is_empty())
        .filter_map(|line| serde_json::from_str::<SparseVersionJson>(line).ok())
        .filter(|v| !v.yanked)
        .map(|v| v.vers)
        .filter(|v| prerelease || v.pre.is_empty())
        .collect::<Vec<_>>();

    // Sparse index entries are ordered oldest to newest
    available.sort_unstable_by(|a, b| b.cmp(a));

    if let Some(req) = version_req {
        let mut newer = vec![];

        for v in &available {
            if req.matches(v) {
                return Ok((v.clone(), newer));
            }

            newer.push(v.clone());
        }

        Err(anyhow!(
            "\
                Failed to find an available version matching the requirement `{req}` \
                (available: {:?})\
            ",
            available
                .iter()
                .take(5)
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        ))
    } else if available.is_empty() {
        Err(anyhow!("Failed to find any available version"))
    } else {
        Ok((available[0].clone(), vec![]))
    }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -

/// Get the active toolchain
pub fn get_active_toolchain() -> Result<String> {
    let shell = Shell {
        print: false,
        ..Default::default()
    };

    let result = shell.run1(&sprint::Command {
        command: String::from("rustup show active-toolchain -v"),
        stdout: Pipe::string(),
        ..Default::default()
    });

    if let Pipe::String(Some(s)) = &result.stdout {
        Ok(s.clone())
    } else {
        Err(anyhow!("Failed to get the active toolchain"))
    }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -

/// Get the rust version from the active toolchain
pub fn get_rust_version() -> Result<semver::Version> {
    get_active_toolchain()?
        .lines()
        .find_map(|line| {
            line.split_whitespace()
                .collect::<Vec<_>>()
                .windows(2)
                .find(|w| w[0] == "rustc")
                .map(|w| w[1])
        })
        .ok_or_else(|| anyhow!("rustc not found"))
        .and_then(parse_to_version)
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -

/// Parse a string to a [`semver::Version`]
pub fn parse_to_version(s: &str) -> Result<semver::Version> {
    match semver::Version::parse(s) {
        Ok(v) => Ok(v),
        Err(e) => Err(anyhow!("Failed to parse version string `{s}`: {e}")),
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
