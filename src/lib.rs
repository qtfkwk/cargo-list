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
}

impl Crates {
    /**
    Deserialize from a `~/.cargo/.crates2.json` file
    */
    pub fn from(path: &Path) -> Result<Crates> {
        let mut crates: Crates = serde_json::from_reader(File::open(path)?)?;
        let errors = crates
            .installs
            .par_iter_mut()
            .filter_map(|(k, v)| v.init(k).err())
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
    Sort crates by whether they are outdated
    */
    pub fn crates(&self) -> (BTreeMap<&str, &Crate>, BTreeMap<&str, &Crate>) {
        (
            self.installs
                .values()
                .map(|x| (x.name.as_str(), x))
                .collect(),
            self.installs
                .values()
                .filter(|x| x.outdated)
                .map(|x| (x.name.as_str(), x))
                .collect(),
        )
    }

    /**
    Returns true if no crates are installed
    */
    pub fn is_empty(&self) -> bool {
        self.installs.is_empty()
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
    rust_version: String,

    #[serde(skip_deserializing)]
    pub outdated: bool,

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
    fn init(&mut self, k: &str) -> Result<()> {
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

        if self.kind == External {
            self.available = cargo_search_version(&self.name, &self.installed)?;
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

fn cargo_search_version(name: &str, installed: &str) -> Result<String> {
    let c = Command::new("cargo")
        .args(["search", "--limit", "1", name])
        .output()?;
    let result = std::str::from_utf8(&c.stdout)?;
    Ok(
        if ["alpha", "beta", "rc"]
            .par_iter()
            .any(|x| result.contains(x))
        {
            installed.to_string()
        } else if let Some(available) = result.split('"').nth(1) {
            available.to_string()
        } else {
            String::from("?")
        },
    )
}
