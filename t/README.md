# Usage

```text
$ cargo list -h
!run:../target/release/cargo-list list -h
```

```text
$ cargo list -V
!run:../target/release/cargo-list list -V
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

Please find the [`CHANGELOG.md`] in the [repository].

[`CHANGELOG.md`]: https://github.com/qtfkwk/cargo-list/blob/main/CHANGELOG.md
[repository]: https://github.com/qtfkwk/cargo-list/

