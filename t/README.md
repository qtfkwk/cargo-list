# CLI

```text
$ cargo list -V
!run:../target/release/cargo-list list -V
```

```text
$ cargo list -h
!run:../target/release/cargo-list list -h
```

```text
$ cargo list --help
!run:../target/release/cargo-list list --help
```

## Protocol

The default protocol used to get the latest available upstream crate versions is the [sparse index].

The previous default protocol was the [REST API], which is still available and `cargo-list` can use
it temporarily via `-p rest-api` or by default by setting `protocol = "rest-api"` in the
configuration file.
Using the REST API for retrieving the latest available versions is quite excessive because it (a)
demands server-side processing, (b) can be done perfectly well via the sparse index, and (c) is
intended for other use cases (like `cargo search`, `cargo info`, etc).

Note that `cargo-list` has never had nor any plans exist to support the legacy Git index.

See also the [Data Access Policy] at Crates.io.

> [!WARNING]
> The REST API does have a limit, will respond with [HTTP 429 Too Many Requests] errors if you
> exceed it, and `cargo list` prints them to stderr and exits with error code 3 when they occur.
> The [published limit](https://crates.io/data-access#api) is "a maximum of 1 request per second,"
> however `cargo-list` typically sends 1--2 requests per external crate every time it runs.
> The 429 errors are not observed until running `cargo list` with enough installed crates and/or
> times to produce several hundred requests inside a short window (1--5 minutes).
> The throttle clears after a few minutes.

[sparse index]:
https://doc.rust-lang.org/cargo/reference/registry-index.html

[REST API]:
https://doc.rust-lang.org/cargo/reference/registry-web-api.html

[Data Access Policy]:
https://crates.io/data-access

[HTTP 429 Too Many Requests]:
https://datatracker.ietf.org/doc/html/rfc6585#section-4

## List installed external crates

```bash
cargo list -e
```

> [!NOTE]
> The `-e`, `-g`, `-l`, and `-a` options provide a flexible and concise way to specify the desired
> crate kinds.
> The order they are used makes no difference; crates are always listed by kind (if included) in
> local, git, external order.
>
> | Options                                                | Description              |
> |--------------------------------------------------------|--------------------------|
> | `-e` or *None*                                         | External only            |
> | `-l`                                                   | Local only               |
> | `-g`                                                   | Git only                 |
> | `-le` or `-el`                                         | Local and external only  |
> | `-ge` or `-eg`                                         | Git and external only    |
> | `-lg` or `-gl`                                         | Local and git only       |
> | `-lge`, `-leg`, `-gle`, `-gel`, `-elg`, `-egl` or `-a` | Local, git, and external |

## List installed external crates containing `cargo`

```bash
cargo list -e cargo
```

## List installed external crates beginning with `cargo`

```bash
cargo list -e ^cargo
```

## List installed external crates ending with `list`

```bash
cargo list -e 'list$'
```

## List installed external crates matching `cargo-list`

```bash
cargo list -e '^cargo-list$'
```

## List outdated external crates

```bash
cargo list -oe
```

## Update outdated external crates

```bash
cargo list -uoe
```

## List the `cargo install` commands to update outdated external crates

```bash
cargo list -nuoe
```

## List outdated external crates (ignore pinned version requirements)

```bash
cargo list -oeI
```

## List outdated external crates (include crates compiled with old Rust)

```bash
cargo list -oeR
```

## List outdated external crates (ignore pinned version requirements and include crates compiled with old Rust)

```bash
cargo list -oeIR
```

## Update outdated external crates (ignore pinned version requirements and include crates compiled with old Rust)

```bash
cargo list -uoeIR
```

## List crates installed via git

```bash
cargo list -g
```

## List installed local crates

```bash
cargo list -l
```

## List installed local, git, and external crates

```bash
cargo list -lge
```

or shorter:

```bash
cargo list -a
```

## List outdated crates installed via git

```bash
cargo list -og
```

## List outdated installed local crates

```bash
cargo list -ol
```

## List outdated installed local, git, and external crates

```bash
cargo list -olge
```

or shorter:

```bash
cargo list -oa
```

## Dump installed external crates to JSON

```bash
cargo list -f json
```

## Dump installed external crates to pretty JSON

```bash
cargo list -f json-pretty
```

## Dump installed external crates to Rust

```bash
cargo list -f rust
```

## Dump installed external crates to pretty Rust

```bash
cargo list -f rust-pretty
```

## Dump outdated installed external crates to JSON

```bash
cargo list -f json -o
```

## Dump outdated installed external crates to pretty JSON

```bash
cargo list -f json-pretty -o
```

## Dump outdated installed external crates to Rust

```bash
cargo list -f rust -o
```

## Dump outdated installed external crates to pretty Rust

```bash
cargo list -f rust-pretty -o
```

## Enable debug logging

```bash
RUST_LOG=reqwest=debug,hyper=debug cargo list
```

## Show running configuration

```bash
cargo list -c
!run:../target/release/cargo-list list -c |sed 's/\/home\/[^\/]*/\/home\/user/'
```

# Library

!inc:LIBRARY.md

# Changelog

Please read the [`CHANGELOG.md`] in the [repository].

[`CHANGELOG.md`]: https://github.com/qtfkwk/cargo-list/blob/main/CHANGELOG.md
[repository]: https://github.com/qtfkwk/cargo-list

