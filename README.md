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
  -R               Consider a crate to be outdated if compiled with a Rust version different than the active
                   toolchain
  -u, --update     Update outdated crates
  -n, --dry-run    Dry run
  -c <PATH>        Cargo install metadata file [default: ~/.cargo/.crates2.json]
  -r, --readme     Print readme
  -h, --help       Print help
  -V, --version    Print version
```

```text
$ cargo list -V
cargo-list 0.16.0
```

### List installed external crates

```bash
cargo list
```

### List outdated external crates

```bash
cargo list -o
```

### Update outdated external crates

```bash
cargo list -ou
```

### List the `cargo install` commands to update outdated external crates

```bash
cargo list -oun
```

### Update outdated external crates (include crates compiled with old Rust)

```bash
cargo list -ouR
```

### List crates installed via git

```bash
cargo list -k git
```

### List installed local crates

```bash
cargo list -k local
```

### List installed local, git, and external crates

```bash
cargo list -k local -k git -k external
```

or shorter:

```bash
cargo list -a
```

### List outdated crates installed via git

```bash
cargo list -k git -o
```

### List outdated installed local crates

```bash
cargo list -k local -o
```

### List outdated installed local, git, and external crates

```bash
cargo list -k local -k git -k external -o
```

or shorter:

```bash
cargo list -ao
```

### Dump installed external crates to JSON

```bash
cargo list -f json
```

### Dump installed external crates to pretty JSON

```bash
cargo list -f json-pretty
```

### Dump installed external crates to Rust

```bash
cargo list -f rust
```

### Dump installed external crates to pretty Rust

```bash
cargo list -f rust-pretty
```

### Dump outdated installed external crates to JSON

```bash
cargo list -f json -o
```

### Dump outdated installed external crates to pretty JSON

```bash
cargo list -f json-pretty -o
```

### Dump outdated installed external crates to Rust

```bash
cargo list -f rust -o
```

### Dump outdated installed external crates to pretty Rust

```bash
cargo list -f rust-pretty -o
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

