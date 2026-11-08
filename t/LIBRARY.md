```rust
use cargo_list::Crates;

match Crates::new(&[], &cargo_list::Kind::all(), None) {
    Ok(crates) => {
        if crates.is_empty() {
            println!("No crates installed!");
        } else {
            let installed = crates.installed();
            let outdated = crates.outdated(false);

            if outdated.is_empty() {
                // List installed crates in CSV
                println!("Name,Installed");
                for (name, c) in installed {
                    println!("{name},{}", c.installed);
                }
            } else {
                // List outdated crates in CSV
                println!("Name,Installed,Available");
                for (name, c) in outdated {
                    println!("{name},{},{}", c.installed, c.available.as_ref().unwrap());
                }

                // Print the `cargo install` commands for outdated crates
                // for (_name, c) in outdated {
                //     println!("{}", c.update_command());
                //     // Update:
                //     // c.update()?;
                // }
            }
        }
    }
    Err(e) => {
        eprintln!("Error: {e}");
    }
}
```

