use cargo_list::Crates;
use expanduser::expanduser;
use rayon::prelude::*;
use std::collections::BTreeMap;

#[test]
fn it_works() {
    let path = expanduser("~/.cargo/.crates2.json").unwrap();

    match Crates::from(&path) {
        Ok(installed) => {
            if installed.is_empty() {
                println!("No crates installed!");
            } else {
                let all = installed.crates();
                let outdated = all
                    .par_iter()
                    .filter_map(|(&name, &c)| c.outdated.then_some((name, c)))
                    .collect::<BTreeMap<_, _>>();

                if outdated.is_empty() {
                    // List all crates in CSV
                    println!("Name,Installed");
                    for (name, c) in &all {
                        println!("{name},{}", c.installed);
                    }
                } else {
                    // List outdated crates in CSV
                    println!("Name,Installed,Available");
                    for (name, c) in &outdated {
                        println!("{name},{},{}", c.installed, c.available);
                    }

                    // Print the `cargo install` commands for outdated crates
                    // for command in outdated
                    //     .iter()
                    //     .map(|(_name, c)| c.update_command().join(" "))
                    // {
                    //     println!("{command}");
                    // }

                    // Update outdated crates
                    // outdated.iter().for_each(|(_name, c)| c.update());
                }
            }
        }
        Err(e) => {
            eprintln!("Error: {e}");
        }
    }
}
