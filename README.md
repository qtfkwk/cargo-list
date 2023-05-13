Improved `cargo install --list`

```text
$ cargo list -h
Improved `cargo install --list`

Usage: cargo-list [OPTIONS]

Options:
  -f <FORMAT>      Output format [default: md] [possible values: json,
                   json-pretty, md, rust, rust-pretty]
      --outdated   Hide up-to-date crates
      --update     Update outdated crates
  -h, --help       Print help
  -V, --version    Print version
```

```text
$ cargo list -V
cargo-list 0.1.0
```

