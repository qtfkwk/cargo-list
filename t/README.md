# Usage

## CLI

```text
$ cargo list -h
!run:../target/release/cargo-list list -h
```

```text
$ cargo list -V
!run:../target/release/cargo-list list -V
```

### List installed external crates

```bash
cargo list
```

### List installed external crates containing `cargo`

```bash
cargo list cargo
```

### List installed external crates beginning with `cargo`

```bash
cargo list ^cargo
```

### List installed external crates ending with `list`

```bash
cargo list 'list$'
```

### List installed external crates matching `cargo-list`

```bash
cargo list '^cargo-list$'
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

### List outdated external crates (ignore version requirements)

```bash
cargo list -oI
```

### List outdated external crates (include crates compiled with old Rust)

```bash
cargo list -oR
```

### Update outdated external crates (ignore version requirements and include crate compiled with old Rust)

```bash
cargo list -oIRu
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

!run:sed 's/^# //' LIBRARY.md

# Changelog

Please read the [CHANGELOG.md] in the [repository].

[CHANGELOG.md]: https://github.com/qtfkwk/cargo-list/blob/main/CHANGELOG.md
[repository]: https://github.com/qtfkwk/cargo-list

