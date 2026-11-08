#![doc = include_str!("../t/LIBRARY.md")]

//--------------------------------------------------------------------------------------------------
// Crates

use {
    anyhow::{Result, anyhow},
    clap::ValueEnum,
    dirs::home_dir,
    indexmap::IndexSet,
    rayon::prelude::*,
    regex::RegexSet,
    reqwest::blocking::Client,
    serde::{Deserialize, Serialize},
    sprint::{Command, Pipe, Shell},
    std::{
        collections::{BTreeMap, BTreeSet},
        fmt::Write,
        fs::File,
        path::PathBuf,
        sync::LazyLock,
    },
    veg::colored::ColoredString,
};

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

static CONFIG_FILE: LazyLock<PathBuf> = LazyLock::new(|| {
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
    /// Get the current state of installed crates
    pub fn new(include: &[&str], kinds: &IndexSet<Kind>) -> Result<Crates> {
        let crates = CratesJson::new()?;
        let config = Config::new(&crates)?;
        let protocol = config.protocol;
        let rust = get_rust_version()?;
        let installed = crates.to_map(include, kinds, &rust, &config)?;

        Ok(Crates {
            protocol,
            installed,
            rust,
        })
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
            Err(anyhow::anyhow!(
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
        let locked = config.is_locked(&name)?;

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
    fn new(crates_json: &CratesJson) -> Result<Config> {
        if CONFIG_FILE.exists() {
            if CONFIG_FILE.is_file() {
                // Deserialize the config file

                let config_str = std::fs::read_to_string(&*CONFIG_FILE).map_err(|e| {
                    anyhow!(
                        "Failed to read config file `{}`: {e}",
                        CONFIG_FILE.display(),
                    )
                })?;

                let config: Config = toml::from_str(&config_str).map_err(|e| {
                    anyhow!(
                        "Failed to deserialize config file `{}`: {e}",
                        CONFIG_FILE.display(),
                    )
                })?;

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
                Err(anyhow::anyhow!(
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

    fn is_locked(&self, name: &str) -> Result<bool> {
        self.crates
            .get(name)
            .ok_or_else(|| anyhow!("Invalid crate name `{name}`"))
            .map(|x| x.locked)
    }

    fn save(&self) -> Result<()> {
        let config_dir = CONFIG_FILE
            .parent()
            .ok_or_else(|| anyhow!("Failed to get directory for `{}`", CONFIG_FILE.display()))?;

        std::fs::create_dir_all(config_dir)
            .map_err(|e| anyhow!("Failed to create directory `{}`: {e}", config_dir.display()))?;

        let config_toml =
            toml::to_string_pretty(self).map_err(|e| anyhow!("Failed to serialize config: {e}"))?;

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

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum Protocol {
    #[default]
    Sparse,
    Api,
    // Git,
}

impl Protocol {
    fn get_latest(
        self,
        name: &str,
        version_req: Option<&semver::VersionReq>,
        prerelease: bool,
    ) -> Result<(semver::Version, Vec<semver::Version>)> {
        match self {
            Protocol::Sparse => sparse_latest(name, version_req, prerelease),
            Protocol::Api => api_latest(name, version_req, prerelease),
            // Protocol::Git => {
            //     unimplemented!()
            // }
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Functions

/**
Get the latest available (not prerelease or yanked) version(s) for a crate, optionally matching a
required version, via the Crates.io REST API
*/
pub fn api_latest(
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

    let result = shell.run1(&Command {
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
