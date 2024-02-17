# build

* clippy
* `README.md`
* test
* doc

```
cargo build --release
```

# `README.md`

* `t/README.md`
* `t/LIBRARY.md`
* `CHANGELOG.md`
* `Cargo.toml`

```
cargo build --release
kapow {0} >{target}
```

# clippy

```
cargo clippy -- -D clippy::all
```

# test

```
cargo test crates_from -- --nocapture
```

# doc

```
cargo doc
```

# check

```
cargo outdated --exit-code 1
cargo audit
```

# update

```
cargo upgrade --incompatible
cargo update
```

# install

```
cargo install --path .
```

# uninstall

```
cargo uninstall $(toml get -r Cargo.toml package.name)
```

# install-deps

```
cargo install cargo-audit cargo-edit cargo-outdated kapow toml-cli
```

