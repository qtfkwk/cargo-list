# CLI

```text
$ cargo list -h
!run:../target/release/cargo-list list -h
```

```text
$ cargo list -V
!run:../target/release/cargo-list list -V
```

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

# Library

!inc:LIBRARY.md

# Changelog

Please read the [`CHANGELOG.md`] in the [repository].

[`CHANGELOG.md`]: https://github.com/qtfkwk/cargo-list/blob/main/CHANGELOG.md
[repository]: https://github.com/qtfkwk/cargo-list

