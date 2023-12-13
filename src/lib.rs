use anyhow::{anyhow, Result};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs::File, path::Path, process::Command};

//--------------------------------------------------------------------------------------------------

#[derive(Debug, Default, Serialize, Eq, PartialEq, Hash, Clone)]
pub enum Kind {
    #[default]
    External,
    Git,
    Local,
}

use Kind::*;

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

#[derive(Debug, Serialize, Deserialize)]
pub struct Crates {
    installs: BTreeMap<String, Crate>,
}

impl Crates {
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

    pub fn is_empty(&self) -> bool {
        self.installs.is_empty()
    }
}

//--------------------------------------------------------------------------------------------------

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

    pub fn update(&self) {
        Command::new("cargo")
            .args(["install", &self.name])
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
