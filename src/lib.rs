use anyhow::Result;
use rayon::prelude::*;
use serde::Serialize;
use std::{process::Command, str::from_utf8};

//--------------------------------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct Crates {
    pub all: Vec<Crate>,
    pub outdated: Vec<Crate>,
}

impl Crates {
    pub fn new() -> Result<Crates> {
        let all = from_utf8(
            &Command::new("cargo")
                .args(["install", "--list"])
                .output()?
                .stdout,
        )?
        .par_lines()
        .filter_map(|x| {
            if x.starts_with(' ') || x.contains('(') {
                None
            } else {
                let mut s = x.split(' ');
                let name = s.next().unwrap();
                let installed = s.next().unwrap().split(':').next().unwrap();
                Crate::new(name, installed).ok()
            }
        })
        .collect::<Vec<_>>();
        let outdated = all
            .iter()
            .filter(|x| x.outdated)
            .cloned()
            .collect::<Vec<_>>();
        Ok(Crates { all, outdated })
    }

    pub fn crates(&self, outdated: bool) -> &[Crate] {
        if outdated {
            &self.outdated
        } else {
            &self.all
        }
    }
}

//--------------------------------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize)]
pub struct Crate {
    pub name: String,
    pub installed: String,
    pub available: String,
    pub outdated: bool,
}

impl Crate {
    pub fn new(name: &str, installed: &str) -> Result<Crate> {
        let available = {
            let c = Command::new("cargo")
                .args(["search", "--limit", "1", name])
                .output()?;
            let search = from_utf8(&c.stdout)?;
            if ["alpha", "beta", "rc"]
                .par_iter()
                .any(|x| search.contains(x))
            {
                installed.to_string()
            } else if let Some(available) = search.split('"').nth(1) {
                format!("v{available}")
            } else {
                String::from("?")
            }
        };
        let outdated = available != installed;
        Ok(Crate {
            name: name.to_string(),
            installed: installed.to_string(),
            available,
            outdated,
        })
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
