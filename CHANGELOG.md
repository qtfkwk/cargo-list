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
* 0.11.0 (2023-11-29): Remove dev dependency on kapow; remove [`pager`] on
  windows
* 0.12.0 (2023-12-13): Use the user's `~/.cargo/.crates2.json` instead of
  running `cargo install --list` and parsing; list local and git crates; enable
  short options; add `-k`, `-a`, `-c` options; remove the `--update-all` option;
  report total number of crates or number of outdated crates; remove examples;
  update dependencies
* 0.13.0 (2023-12-13): Fix the `cargo install` command to update a crate; add
  library docstrings; general cleanup
    * 0.13.1 (2023-12-13): Replace the changelog in the readme with a link
* 0.14.0 (2023-12-14): Add library usage example to readme, module doc, and
  integration test
    * 0.14.1 (2023-12-14): Fix external crate count
    * 0.14.2 (2023-12-14): Fix another external crate count
* 0.15.0 (2023-12-14): Clarify and expose the `latest` function; split the
  `Crates::crates` method to enable getting views of `all` or `outdated` crates
  separately; improve doc

[`atty`]: https://crates.io/crates/atty
[`bunt`]: https://crates.io/crates/bunt
[`clap`]: https://crates.io/crates/clap
[`colored`]: https://crates.io/crates/colored
[`is-terminal`]: https://crates.io/crates/is-terminal
[`kapow`]: https://crates.io/crates/kapow
[`mkrs`]: https://crates.io/crates/mkrs
[`pager`]: https://crates.io/crates/pager

[`Makefile.md`]: Makefile.md

