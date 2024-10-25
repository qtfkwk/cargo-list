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
* 0.16.0 (2023-12-15): Add the `-R` and `-n` options; add CLI examples to readme
* 0.17.0 (2023-12-15): Replace `Crates`' repetitive `all` and `outdated` methods
  with a better `crates` method; display the active toolchain with `-R`; replace
  the `Crate::update` method with a `run` function and halt if an update fails;
  add the `-I` option; improve the `latest` function to process an optional
  version requirement and use the REST API instead of the `cargo search` command
  to get all available versions
* 0.18.0 (2023-12-18): Replace markdown output lists with colorized tables using
  [`veg`]; eliminate [`bunt`] and [`is-terminal`] dependencies and use
  `veg::colored` re-exported [`colored`] `ColoredString` and `Colorized`
  instead; update dependencies
    * 0.18.1 (2023-12-18): Don't print an empty table; update dependencies
* 0.19.0 (2023-12-19): Add spinner; update dependencies
* 0.20.0 (2023-12-24): Use [`sprint`] shell; update dependencies
    * 0.20.1 (2023-12-24): Update dependencies
    * 0.20.2 (2023-12-26): Update dependencies
    * 0.20.3 (2023-12-27): Update dependencies
    * 0.20.4 (2023-12-30): Fix spinner clear; update dependencies
    * 0.20.5 (2024-01-05): Update dependencies
    * 0.20.6 (2024-01-05): Update dependencies
* 0.21.0 (2024-01-06): Number rows; update dependencies
* 0.22.0 (2024-01-06): Replace notes column with optional rust version column
  via `-R`
    * 0.22.1 (2024-01-06): Fix missing rust version if outdated
* 0.23.0 (2024-01-06): Add level 2 heading for each crate updated
    * 0.23.1 (2024-01-24): Fix message after updating; update dependencies
* 0.24.0 (2024-02-17): Add pattern match include feature and
  `Crates::from_include` method via [`regex`]; update dependencies
    * 0.24.1 (2024-03-11): Update dependencies
* 0.25.0 (2024-04-14): Print a better summary after updating; update
  dependencies
    * 0.25.1 (2024-07-30): Fix makefile; update dependencies
    * 0.25.2 (2024-08-15): Update dependencies
    * 0.25.3 (2024-08-22): Fix readme; add `commit` target to makefile; update dependencies
* 0.26.0 (2024-10-24): Add clap color; update dependencies

[`atty`]: https://crates.io/crates/atty
[`bunt`]: https://crates.io/crates/bunt
[`clap`]: https://crates.io/crates/clap
[`colored`]: https://crates.io/crates/colored
[`is-terminal`]: https://crates.io/crates/is-terminal
[`kapow`]: https://crates.io/crates/kapow
[`mkrs`]: https://crates.io/crates/mkrs
[`pager`]: https://crates.io/crates/pager
[`regex`]: https://crates.io/crates/regex
[`sprint`]: https://crates.io/crates/sprint
[`veg`]: https://crates.io/crates/veg

[`Makefile.md`]: Makefile.md

