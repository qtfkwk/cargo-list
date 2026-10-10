#![doc = include_str!("../t/LIBRARY.md")]

//--------------------------------------------------------------------------------------------------
// Crates

use {
    anyhow::{Context, Result, anyhow},
    dirs::home_dir,
    rayon::prelude::*,
    regex::RegexSet,
    reqwest::blocking::Client,
    serde::{Deserialize, Serialize},
    sprint::{Command, Pipe, Shell},
    std::{
        collections::{BTreeMap, BTreeSet},
        fs::File,
        path::{Path, PathBuf},
        sync::LazyLock,
    },
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

//--------------------------------------------------------------------------------------------------
// Enums

/// Crate kind
#[derive(Debug, Default, Serialize, Eq, PartialEq, Hash, Clone)]
pub enum Kind {
    Local,
    Git,

    #[default]
    External,
}

use Kind::{External, Git, Local};

/// All crate kinds
pub const ALL_KINDS: [Kind; 3] = [Local, Git, External];

impl Kind {
    fn from(source: &str) -> Kind {
        if source.starts_with("git+") {
            Git
        } else if source.starts_with("path+") {
            Local
        } else {
            External
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Structs

/// All installed crates
#[derive(Debug, Serialize, Deserialize)]
pub struct Crates {
    installs: BTreeMap<String, Crate>,

    #[serde(skip)]
    pub active_toolchain: String,

    #[serde(skip)]
    pub active_version: String,
}

impl Crates {
    /**
    Deserialize from a `~/.cargo/.crates2.json` file and process each crate in
    parallel to:

    * Parse the name, version, source, rust version
    * Get the latest avaiable version
    * Determine the crate type

    # Errors

    Returns an error if not able to read the file at the given path
    */
    pub fn from(path: &Path) -> Result<Crates> {
        Crates::from_include(path, &[])
    }

    /**
    Return true if no crates are installed
    */
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.installs.is_empty()
    }

    /// Return a view of all crates
    #[must_use]
    pub fn crates(&self) -> BTreeMap<&str, &Crate> {
        self.installs
            .values()
            .map(|x| (x.name.as_str(), x))
            .collect()
    }

    /**
    Like the [`Crates::from`] method, but accepts zero or more include patterns to match against
    crate names

    # Errors

    Returns an error if not able to read the file at the given path or a pattern is not a valid
    regular expression
    */
    #[allow(clippy::missing_panics_doc)]
    pub fn from_include(path: &Path, patterns: &[&str]) -> Result<Crates> {
        let mut crates: Crates = serde_json::from_reader(File::open(path)?)?;
        if !patterns.is_empty() {
            let set = RegexSet::new(patterns)?;
            crates.installs = crates
                .installs
                .into_par_iter()
                .filter_map(|(k, v)| {
                    if set.is_match(k.split_once(' ').unwrap().0) {
                        Some((k, v))
                    } else {
                        None
                    }
                })
                .collect();
        }
        crates.active_toolchain = active_toolchain();
        crates.active_version = crates
            .active_toolchain
            .lines()
            .filter_map(|line| {
                line.split(' ')
                    .skip_while(|&word| word != "rustc")
                    .nth(1)
                    .map(ToString::to_string)
            })
            .nth(0)
            .unwrap();
        let errors = crates
            .installs
            .par_iter_mut()
            .filter_map(|(k, v)| {
                v.init(k, &crates.active_version)
                    .with_context(|| format!("Failed to process crate '{k}'"))
                    .err()
            })
            .collect::<Vec<_>>();
        if errors.is_empty() {
            Ok(crates)
        } else {
            Err(anyhow!(format!(
                "Errors: {}",
                errors
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            )))
        }
    }

    /// Return crate options
    #[must_use]
    pub fn config(&self) -> BTreeMap<String, CrateOptions> {
        self.installs
            .values()
            .map(|x| (x.name.clone(), x.options()))
            .collect()
    }

    /**
    Save configuration file

    # Errors

    Returns an error if fails to serialize crate options or write the configuration file.
    */
    pub fn save(&self, path: &Path) -> Result<()> {
        match toml::to_string(&self.config()) {
            Ok(content) => match std::fs::write(path, &content) {
                Ok(()) => Ok(()),
                Err(e) => Err(anyhow!(
                    "Failed to write configuration file `{}`: `{e}`",
                    path.display(),
                )),
            },
            Err(e) => Err(anyhow!("Failed to serialize to TOML: `{e}`")),
        }
    }

    /// Set locked crates from a deserialized configuration file
    pub fn set_locked(&mut self, config: &BTreeMap<String, CrateOptions>) {
        for c in self.installs.values_mut() {
            if let Some(o) = config.get(c.name.as_str()) {
                c.locked = o.locked;
            }
        }
    }

    /**
    Lock crates

    # Errors

    Returns an error if a crate name is not valid.
    */
    pub fn set_locks(&mut self, crates: &BTreeSet<String>) -> Result<()> {
        for c in self.installs.values_mut() {
            if crates.contains(&c.name) {
                c.locked = true;
            }
        }

        Ok(())
    }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -

/// Individual installed crate
#[derive(Debug, Serialize, Deserialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct Crate {
    #[serde(skip)]
    pub name: String,

    #[serde(default)]
    locked: bool,

    #[serde(skip_deserializing)]
    pub kind: Kind,

    #[serde(skip)]
    pub installed: String,

    #[serde(skip)]
    installed_: Option<semver::Version>,

    #[serde(skip_deserializing)]
    prerelease: bool,

    #[serde(skip)]
    pub available: String,

    #[serde(skip)]
    pub newer: Vec<String>,

    #[serde(skip)]
    pub rust_version: String,

    #[serde(skip)]
    pub outdated: bool,

    #[serde(skip)]
    pub outdated_rust: bool,

    #[serde(skip)]
    source: String,

    #[serde(skip_serializing)]
    pub version_req: Option<String>,

    bins: Vec<String>,
    features: Vec<String>,
    all_features: bool,
    no_default_features: bool,
    profile: String,
    target: String,
    rustc: Option<String>,
}

impl Crate {
    /// Initialize additional fields after deserialization
    fn init(&mut self, k: &str, active_version: &str) -> Result<()> {
        let mut s = k.split(' ');
        self.name = s.next().unwrap().to_string();
        self.installed = s.next().unwrap().to_string();
        self.source = s
            .next()
            .unwrap()
            .strip_prefix('(')
            .unwrap()
            .strip_suffix(')')
            .unwrap()
            .to_string();
        self.installed_ = semver::Version::parse(&self.installed).ok();
        self.prerelease = self.installed_.as_ref().is_some_and(|x| !x.pre.is_empty());

        self.kind = Kind::from(&self.source);

        self.rust_version = if let Some(s) = &self.rustc {
            s.strip_prefix("rustc ")
                .unwrap()
                .split_once(' ')
                .unwrap()
                .0
                .to_string()
        } else {
            String::from("?")
        };

        self.outdated_rust = self.rust_version != active_version;

        if self.kind == External {
            (self.available, self.newer) = latest(&self.name, &self.version_req, self.prerelease)?;
            self.outdated = self.installed != self.available;
        }

        Ok(())
    }

    /**
    Generate the cargo install command to update the crate

    # Panics

    Panics if a git crate and not able to find `#` in the source
    */
    pub fn update_command(&self, pinned: bool) -> Vec<String> {
        let mut r = vec!["cargo", "install"];

        if self.no_default_features {
            r.push("--no-default-features");
        }

        let features = if self.features.is_empty() {
            None
        } else {
            Some(self.features.join(","))
        };
        if let Some(features) = &features {
            r.push("-F");
            r.push(features);
        }

        if !pinned && let Some(version) = &self.version_req {
            r.push("--version");
            r.push(version);
        }

        r.push("--profile");
        r.push(&self.profile);

        r.push("--target");
        r.push(&self.target);

        if self.outdated_rust {
            r.push("--force");
        }

        if self.kind == Git {
            r.push("--git");
            r.push(&self.source[4..self.source.find('#').unwrap()]);
            for bin in &self.bins {
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

    fn options(&self) -> CrateOptions {
        CrateOptions {
            locked: self.locked,
        }
    }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -

#[derive(Deserialize, Serialize)]
pub struct CrateOptions {
    locked: bool,
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -

/**
Deserialize the crate version object returned via the crates.io API
(`https://crates.io/api/v1/crates/{name}/versions`) in the [`latest()`] function
*/
#[derive(Debug, Deserialize)]
struct Versions {
    versions: Vec<Version>,
}

impl Versions {
    fn iter(&self) -> std::slice::Iter<'_, Version> {
        self.versions.iter()
    }

    fn available(&self, prerelease: bool) -> Vec<&Version> {
        self.iter().filter(|x| x.is_available(prerelease)).collect()
    }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -

#[derive(Debug, Deserialize)]
struct Version {
    num: semver::Version,
    yanked: bool,
}

impl Version {
    fn is_available(&self, prerelease: bool) -> bool {
        if !prerelease && !self.num.pre.is_empty() {
            return false;
        }
        !self.yanked
    }
}

//--------------------------------------------------------------------------------------------------
// Functions

/**
Get the latest available (not prerelease or yanked) version(s) for a crate, optionally matching a
required version

# Errors

Returns an error if not able to get the versions via the REST API
*/
pub fn latest(
    name: &str,
    version_req: &Option<String>,
    prerelease: bool,
) -> Result<(String, Vec<String>)> {
    let url = format!("https://crates.io/api/v1/crates/{name}/versions");
    let res = CLIENT.get(&url).send()?;
    let res = res.error_for_status()?;
    let versions = res.json::<Versions>()?;
    let available = versions.available(prerelease);
    if let Some(req_str) = version_req {
        let req = semver::VersionReq::parse(req_str)?;
        let mut newer = vec![];
        for v in &available {
            if req.matches(&v.num) {
                return Ok((v.num.to_string(), newer));
            }
            newer.push(v.num.to_string());
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
                Failed to find an available version matching the requirement `{req_str}` \
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
        Ok((available[0].num.to_string(), vec![]))
    }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -

/// Get the active toolchain
#[must_use]
pub fn active_toolchain() -> String {
    let r = Shell {
        print: false,
        ..Default::default()
    }
    .run(&[Command {
        command: String::from("rustup show active-toolchain -v"),
        stdout: Pipe::string(),
        ..Default::default()
    }]);
    if let Pipe::String(Some(s)) = &r[0].stdout {
        s.clone()
    } else {
        String::new()
    }
}

// - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -

/**
Expand a path with an optional tilde (`~`)

# Panics

Panics if not able to get the user's home directory
*/
#[must_use]
pub fn expanduser(path: &str) -> PathBuf {
    if path == "~" {
        home_dir().unwrap().join(&path[1..])
    } else if path.starts_with('~') {
        home_dir().unwrap().join(&path[2..])
    } else {
        PathBuf::from(&path)
    }
}
