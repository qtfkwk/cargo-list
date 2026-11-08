//! Integration tests

use {anyhow::Result, cargo_list, std::collections::BTreeMap};

#[test]
fn get_active_toolchain() -> Result<()> {
    let active_toolchain = cargo_list::get_active_toolchain()?;

    assert!(!active_toolchain.is_empty());

    println!("active_toolchain = {active_toolchain:?}");

    Ok(())
}

#[test]
fn get_rust_version() -> Result<()> {
    let rust_version = cargo_list::get_rust_version()?;

    println!("rust_version = {rust_version:?}");

    Ok(())
}

#[test]
fn crates_new() -> Result<()> {
    let crates = cargo_list::Crates::new(&[], &cargo_list::Kind::all())?;

    assert!(!crates.is_empty());

    println!(
        "{}",
        serde_json::to_string_pretty(&BTreeMap::from([("crates", crates)]))?,
    );

    Ok(())
}
