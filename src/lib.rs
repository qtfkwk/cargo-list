#![doc = include_str!("../t/LIBRARY.md")]

//--------------------------------------------------------------------------------------------------

use anyhow::{anyhow, Result};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs::File, path::Path, process::Command};

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
    pub current_rust: String,
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
        let mut crates: Crates = serde_json::from_reader(File::open(path)?)?;
        crates.current_rust = active_rust();
        let errors = crates
            .installs
            .par_iter_mut()
            .filter_map(|(k, v)| v.init(k, &crates.current_rust).err())
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

    /**
    Return true if no crates are installed
    */
    pub fn is_empty(&self) -> bool {
        self.installs.is_empty()
    }

    /**
    Return a view of all crates
    */
    pub fn all(&self) -> BTreeMap<&str, &Crate> {
        self.installs
            .values()
            .map(|x| (x.name.as_str(), x))
            .collect()
    }

    /**
    Return a view of outdated crates
    */
    pub fn outdated(&self) -> BTreeMap<&str, &Crate> {
        self.installs
            .values()
            .filter(|x| x.outdated)
            .map(|x| (x.name.as_str(), x))
            .collect()
    }

    /**
    Return a view of external crates compiled with outdated Rust
    */
    pub fn outdated_rust(&self) -> BTreeMap<&str, &Crate> {
        self.installs
            .values()
            .filter(|x| x.outdated_rust && x.kind == External)
            .map(|x| (x.name.as_str(), x))
            .collect()
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
    pub rust_version: String,

    #[serde(skip_deserializing)]
    pub outdated: bool,

    #[serde(skip_deserializing)]
    pub outdated_rust: bool,

    #[serde(skip_deserializing)]
    source: String,

    version_req: Option<String>,
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
    fn init(&mut self, k: &str, current_rust: &str) -> Result<()> {
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

        self.outdated_rust = self.rust_version != current_rust;

        if self.kind == External {
            self.available = latest(&self.name)?.unwrap_or_else(|| self.installed.clone());
            self.outdated = self.installed != self.available;
        }

        Ok(())
    }

    /**
    Generate the cargo install command to update the crate
    */
    pub fn update_command(&self) -> Vec<String> {
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

        r.push("--profile");
        r.push(&self.profile);

        r.push("--target");
        r.push(&self.target);

        if self.outdated_rust {
            r.push("--force");
        }

        r.push(&self.name);

        r.into_iter().map(String::from).collect()
    }

    /**
    Update the crate
    */
    pub fn update(&self) {
        Command::new("cargo")
            .args(&self.update_command()[1..])
            .spawn()
            .unwrap()
            .wait()
            .unwrap();
    }
}

//--------------------------------------------------------------------------------------------------

/**
Get the latest available version for a crate via `cargo search`
*/
pub fn latest(name: &str) -> Result<Option<String>> {
    let result = std::str::from_utf8(
        &Command::new("cargo")
            .args(["search", "--limit", "1", name])
            .output()?
            .stdout,
    )?
    .to_string();
    Ok(result.split('"').nth(1).and_then(|available| {
        if ["alpha", "beta", "rc"]
            .par_iter()
            .any(|x| available.contains(x))
        {
            None
        } else {
            Some(available.to_string())
        }
    }))
}

/**
Get the Rust version of the active toolchain
*/
pub fn active_rust() -> String {
    std::str::from_utf8(
        &Command::new("rustup")
            .args(["show", "active-toolchain", "-v"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .split('\n')
    .nth(1)
    .unwrap()
    .split(' ')
    .nth(1)
    .unwrap()
    .to_string()
}
