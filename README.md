# Usage

## CLI

```text
$ cargo list -h
List and update installed crates

Usage: cargo list [OPTIONS]

Options:
  -f <FORMAT>      Output format [default: md] [possible values: json, json-pretty, md, rust, rust-pretty]
  -k <KIND>        Kind(s) [default: external] [possible values: local, git, external]
  -a               All kinds
  -o, --outdated   Hide up-to-date crates
  -u, --update     Update outdated crates
  -c <PATH>        Cargo install metadata file [default: ~/.cargo/.crates2.json]
  -r, --readme     Print readme
  -h, --help       Print help
  -V, --version    Print version
```

```text
$ cargo list -V
cargo-list 0.15.0
```

## Library

```rust
use cargo_list::Crates;
use expanduser::expanduser;

let path = expanduser("~/.cargo/.crates2.json").unwrap();

match Crates::from(&path) {
    Ok(installed) => {
        if installed.is_empty() {
            println!("No crates installed!");
        } else {
            let outdated = installed.outdated();

            if outdated.is_empty() {
                // List all crates in CSV
                let all = installed.all();
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
```

# Changelog

Please read the [CHANGELOG.md] in the [repository].

[CHANGELOG.md]: https://github.com/qtfkwk/cargo-list/blob/main/CHANGELOG.md
[repository]: https://github.com/qtfkwk/cargo-list

