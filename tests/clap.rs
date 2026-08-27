//! clap's `value_parser!` prefers `From<OsString>`/`From<&OsStr>`/`From<String>`/`From<&str>`
//! over `FromStr`. If any such impl exists, CLI arguments silently skip expansion.

use clap::{Parser, error::ErrorKind};
use expanded_pathbuf::ExpandedPathBuf;
use std::path::Path;

#[derive(Parser)]
struct Cli {
    #[arg(long)]
    path: ExpandedPathBuf,
}

#[test]
fn rejects_unset_variable() {
    let err = Cli::try_parse_from(["app", "--path", "$__EXPANDED_PATHBUF_TEST_UNSET__/x"])
        .err()
        .expect("an unset variable must fail argument parsing");
    assert_eq!(err.kind(), ErrorKind::ValueValidation);
}

#[test]
fn expands_argument() {
    let path = std::env::var_os("PATH").expect("PATH is always set");
    let cli = Cli::try_parse_from(["app", "--path", "$PATH"]).unwrap();
    assert_eq!(cli.path.as_path(), Path::new(&path));
}
