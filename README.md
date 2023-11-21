# Usage

```text
$ cargo list -h
List and update installed crates

Usage: cargo list [OPTIONS]

Options:
  -f <FORMAT>       Output format [default: md] [possible values: json, json-pretty, md, rust, rust-pretty]
      --outdated    Hide up-to-date crates
      --update      Update outdated crates
      --update-all  Force reinstall all crates
  -r, --readme      Print the readme
  -h, --help        Print help
  -V, --version     Print version
```

```text
$ cargo list -V
cargo-list 0.10.1
```

# Examples

## List installed crates

```text
$ cargo list
* bat: v0.23.0
* kapow: v2.9.1 => v2.10.0
```

## List installed crates in JSON

```text
$ cargo list -f json
[{"name":"bat","installed":"v0.23.0","available":"v0.23.0",\
"outdated":false},{"name":"kapow","installed":"v2.9.1","ava\
ilable":"v2.10.0","outdated":true}]
```

## List outdated crates

```text
$ cargo list --outdated
* kapow: v2.9.1 => v2.10.0
```

## List outdated crates in pretty-printed JSON

```text
$ cargo list --outdated -f json-pretty
[
  {
    "name": "kapow",
    "installed": "v2.9.1",
    "available": "v2.10.0",
    "outdated": true
  }
]
```

## Update outdated crates

~~~text
$ cargo list --update
* bat: v0.23.0
* kapow: v2.9.1 => v2.10.0

```text
$ cargo install kapow
    Updating crates.io index
  Installing kapow v2.10.0
...
   Compiling kapow v2.10.0
    Finished release [optimized] target(s) in 7.22s
   Replacing /home/qtfkwk/.cargo/bin/kapow
    Replaced package `kapow v2.9.1` with `kapow v2.10.0` (e\
xecutable `kapow`)
```
~~~

## List installed crates after updating

```text
$ cargo list
* bat: v0.23.0
* kapow: v2.10.0

*All crates are up-to-date!*
```

## List installed crates in JSON after updating

```text
$ cargo list -f json
[{"name":"bat","installed":"v0.23.0","available":"v0.23.0",\
"outdated":false},{"name":"kapow","installed":"v2.10.0","av\
ailable":"v2.10.0","outdated":false}]
```

## List installed crates in pretty-printed JSON after updating

```text
$ cargo list -f json-pretty
[
  {
    "name": "bat",
    "installed": "v0.23.0",
    "available": "v0.23.0",
    "outdated": false
  },
  {
    "name": "kapow",
    "installed": "v2.10.0",
    "available": "v2.10.0",
    "outdated": false
  },
]
```

## List outdated crates after updating

```text
$ cargo list --outdated
*All crates are up-to-date!*
```

## List outdated crates in JSON after updating

```text
$ cargo list --outdated -f json
[]
```

# Changelog

* 0.1.0 (2023-05-13): Initial release
* 0.2.0 (2023-05-13): Replace [`colored`] dependency with [`bunt`]; use [`clap`]
  subcommand
* 0.3.0 (2023-05-14): Add `Crates::crates()` method to simplify usage; update
  dependencies; add examples to readme; add changelog; change description
* 0.3.1 (2023-05-14): Fix readme
* 0.3.2 (2023-05-14): Clean up; remove old dependency [`pager`] *yanked*
* 0.3.3 (2023-05-14): Fix version
* 0.4.0 (2023-05-15): Clean up; disable colors if stdout is not a TTY
* 0.5.0 (2023-05-15): Replace [`atty`] dependency with [`is-terminal`]; fix
  readme
* 0.5.1 (2023-05-16): Fix readme; update dependencies
* 0.5.2 (2023-05-19): Update dependencies
* 0.5.3 (2023-05-19): Fix readme
* 0.6.0 (2023-05-24): Fix changelog and readme; update dependencies
* 0.7.0 (2023-05-24): Add `--readme` option
* 0.8.0 (2023-10-08): Add `--update-all` option and `Crate.update_force()`
  method; update dependencies
* 0.8.1 (2023-10-08): Fix readme
* 0.9.0 (2023-11-06): Miscellaneous fixes for recent changes to [`kapow`]; added
  [`Makefile.md`] for [`mkrs`]; updated dependencies
* 0.10.0 (2023-11-21): Add summary after updates; update dependencies
* 0.10.1 (2023-11-21): Fix readme/changelog

[`atty`]: https://crates.io/crates/atty
[`bunt`]: https://crates.io/crates/bunt
[`clap`]: https://crates.io/crates/clap
[`colored`]: https://crates.io/crates/colored
[`is-terminal`]: https://crates.io/crates/is-terminal
[`kapow`]: https://crates.io/crates/kapow
[`mkrs`]: https://crates.io/crates/mkrs
[`pager`]: https://crates.io/crates/pager

[`Makefile.md`]: Makefile.md

