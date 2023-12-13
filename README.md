# Usage

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
cargo-list 0.13.1
```

# Changelog

Please read the [CHANGELOG.md] in the [repository].

[CHANGELOG.md]: https://github.com/qtfkwk/cargo-list/blob/main/CHANGELOG.md
[repository]: https://github.com/qtfkwk/cargo-list

