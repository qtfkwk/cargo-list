#![doc = include_str!("../t/LIBRARY.md")]

//--------------------------------------------------------------------------------------------------

use anyhow::{anyhow, Result};
use rayon::prelude::*;
use regex::RegexSet;
use serde::{Deserialize, Serialize};
use sprint::*;
use std::{collections::BTreeMap, fs::File, path::Path};

//--------------------------------------------------------------------------------------------------

/**
Crate kind
*/
#[derive(Debug, Default, Serialize, Eq, PartialEq, Hash, Clone)]
pub enum Kind {
    Local,
    Git,

    #[default]
    External,
}

use Kind::*;

/**
All crate kinds
*/
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

/**
All installed crates
*/
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
    */
    pub fn from(path: &Path) -> Result<Crates> {
        Crates::from_include(path, &[])
    }

    /**
    Return true if no crates are installed
    */
    pub fn is_empty(&self) -> bool {
        self.installs.is_empty()
    }

    /**
    Return a view of all crates
    */
    pub fn crates(&self) -> BTreeMap<&str, &Crate> {
        self.installs
            .values()
            .map(|x| (x.name.as_str(), x))
            .collect()
    }

    /**
    Like the [`Crates::from`] method, but accepts zero or more include patterns to match against
    crate names
    */
    pub fn from_include(path: &Path, patterns: &[&str]) -> Result<Crates> {
        let mut crates: Crates = serde_json::from_reader(File::open(path)?)?;
        if !patterns.is_empty() {
            let set = RegexSet::new(patterns).unwrap();
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
            .split('\n')
            .nth(1)
            .unwrap()
            .split(' ')
            .nth(1)
            .unwrap()
            .to_string();
        let errors = crates
            .installs
            .par_iter_mut()
            .filter_map(|(k, v)| v.init(k, &crates.active_version).err())
            .collect::<Vec<_>>();
        if errors.is_empty() {
            Ok(crates)
        } else {
            Err(anyhow!(format!(
                "Errors: {}",
                errors
                    .iter()
                    .map(|x| x.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )))
        }
    }
}

//--------------------------------------------------------------------------------------------------

/**
Individual installed crate
*/
#[derive(Debug, Serialize, Deserialize)]
pub struct Crate {
    #[serde(skip_deserializing)]
    pub name: String,

    #[serde(skip_deserializing)]
    pub kind: Kind,

    #[serde(skip_deserializing)]
    pub installed: String,

    #[serde(skip_deserializing)]
    pub available: String,

    #[serde(skip_deserializing)]
    pub newer: Vec<String>,

    #[serde(skip_deserializing)]
    pub rust_version: String,

    #[serde(skip_deserializing)]
    pub outdated: bool,

    #[serde(skip_deserializing)]
    pub outdated_rust: bool,

    #[serde(skip_deserializing)]
    source: String,

    pub version_req: Option<String>,
    bins: Vec<String>,
    features: Vec<String>,
    all_features: bool,
    no_default_features: bool,
    profile: String,
    target: String,
    rustc: String,
}

impl Crate {
    /**
    Initialize additional fields after deserialization
    */
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

        self.kind = Kind::from(&self.source);

        self.rust_version = self
            .rustc
            .strip_prefix("rustc ")
            .unwrap()
            .split_once(' ')
            .unwrap()
            .0
            .to_string();

        self.outdated_rust = self.rust_version != active_version;

        if self.kind == External {
            (self.available, self.newer) = latest(&self.name, &self.version_req)?;
            self.outdated = self.installed != self.available;
        }

        Ok(())
    }

    /**
    Generate the cargo install command to update the crate
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

        if !pinned {
            if let Some(version) = &self.version_req {
                r.push("--version");
                r.push(version);
            }
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

        r.into_iter().map(String::from).collect()
    }
}

//--------------------------------------------------------------------------------------------------

/**
Get the latest available version(s) for a crate, optionally matching a required version
*/
pub fn latest(name: &str, version_req: &Option<String>) -> Result<(String, Vec<String>)> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("cargo-list")
        .build()?;
    let url = format!("https://crates.io/api/v1/crates/{name}/versions");
    let res = client.get(url).send()?;
    let json = res.json::<serde_json::Value>()?;
    if let Some(available) = json["versions"].as_array() {
        let available = available
            .iter()
            .filter_map(|x| {
                if let Some(version) = x["num"].as_str() {
                    if let Ok(v) = semver::Version::parse(version) {
                        if v.pre.is_empty() {
                            Some(v)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        if let Some(req) = version_req {
            let req = semver::VersionReq::parse(req)?;
            let mut newer = vec![];
            for v in &available {
                if req.matches(v) {
                    return Ok((v.to_string(), newer));
                } else {
                    newer.push(v.to_string());
                }
            }
            Err(anyhow!(
                "Failed to find an available version matching the requirement"
            ))
        } else {
            Ok((available[0].to_string(), vec![]))
        }
    } else {
        Err(anyhow!("Failed to parse versions"))
    }
}

/**
Get the active toolchain
*/
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
        s.to_string()
    } else {
        String::new()
    }
}
