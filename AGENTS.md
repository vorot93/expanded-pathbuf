# expanded-pathbuf — working notes

Conventions and gotchas that cannot be re-derived from the code. User-facing behaviour lives in
`README.md`, which is also the crate-level rustdoc (`#![doc = include_str!]`), so keep it exact.

## Tooling

- The supported toolchain is the latest stable: no `rust-version` is declared and CI does not test
  older compilers, on purpose.
- `Cargo.lock` is not committed, on purpose: this is a library, consumers resolve their own
  lockfile, and CI resolving fresh on every run is what surfaces breakage from new dependency
  releases. `Swatinem/rust-cache` still helps without one — it is keyed on `Cargo.toml`, so a new
  dependency release rebuilds only that crate.
- Format with `cargo fmt` (stable, default configuration).
- Lint with `cargo clippy --workspace --all-targets -- -D warnings` (`--all-targets` is what makes
  the tests get linted) and `RUSTDOCFLAGS='-D warnings' cargo doc --no-deps`.
  `#![warn(missing_docs)]` is on, so every public item needs documentation.
- There are no Cargo features, so there is no feature matrix (`cargo hack`) in CI.

## Design decisions

- **Every constructor expands, and nothing else constructs.** There is no `From<_>` impl at all
  (not even `From<PathBuf>`), no `DerefMut`, and the field is private. clap's `value_parser!`
  prefers `From<OsString>`, `From<&OsStr>`, `From<String>` and `From<&str>` over `FromStr`
  (autoref specialisation in `clap_builder::builder::value_parser`), so any of those impls would
  silently switch expansion off for CLI arguments; a public field or `DerefMut` would let callers
  put unexpanded text into a value after the fact. `tests/clap.rs` and the `compile_fail` doctests
  at the bottom of `src/lib.rs` pin this. A `TryFrom` impl for a new input type is fine; a `From`
  impl is not.
- `Deref<Target = Path>` rather than `PathBuf`, so no mutation route exists (`push`,
  `set_file_name`, `*path = …`). `into_path_buf` and `From<ExpandedPathBuf> for PathBuf` are the
  exits.
- Expansion runs on `OsStr` bytes through `shellexpand::path::full_with_context` with
  `std::env::var_os` as the lookup. `shellexpand::full` is `&str`-only, and even
  `shellexpand::path::full` looks variables up with `std::env::var`, which fails on non-UTF-8
  values — don't switch back to either.
- `shellexpand` silently leaves `~` in place when its `home_dir` callback answers `None`.
  `expand` records that the callback was consulted and answered `None`, and turns that into
  `ExpandError::HomeDirUnavailable`. Unset variables are errors too (the lookup adapter returns
  `Err`, which shellexpand propagates; `Ok(None)` would mean "leave `$NAME` literal"). Passthrough
  was rejected because a literal `~` or `$NAME` in a path becomes a directory with that name.
- `ExpandError` is a crate-owned `thiserror` enum. `anyhow::Error` was rejected because it does
  not implement `std::error::Error` (so the crate would not compose with `Box<dyn Error>`-style
  consumers); `shellexpand::LookupError` was rejected because it would make `shellexpand` a public
  dependency and cannot express the missing-home case.
- Not supported and deliberately left literal (documented in `README.md`): `~user`, Windows
  `%NAME%`, and re-expansion of `~`/`$` inside variable values or `${NAME:-default}` defaults.
  Re-expansion would need a real parser instead of shellexpand's single pass and was judged not
  worth hand-rolling; `%NAME%` would give the same input different meanings per platform.

## Tests

- Inject the home directory and variables through `ExpandedPathBuf::with_context`. Never mutate
  the process environment in tests: `std::env::set_var` is `unsafe` in edition 2024 and races with
  the parallel test harness. The only tests that touch the real environment use a variable that
  always exists (`PATH`) or one that never does.
- The clap test depends on clap's `value_parser!` precedence; if clap changes it, the test tells
  you which trait clap started picking.
