use expanded_pathbuf::{ExpandError, ExpandedPathBuf};
use std::{
    collections::{HashMap, HashSet},
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
};

const HOME: &str = "/home/test";

fn lookup(name: &str) -> Option<&'static str> {
    match name {
        "VAR" => Some("value"),
        "EMPTY" => Some(""),
        "TILDE" => Some("~/from-var"),
        "DOLLAR" => Some("$VAR"),
        _ => None,
    }
}

fn expand(input: impl AsRef<Path>) -> Result<ExpandedPathBuf, ExpandError> {
    ExpandedPathBuf::with_context(input, Some(Path::new(HOME)), lookup)
}

fn expand_without_home(input: &str) -> Result<ExpandedPathBuf, ExpandError> {
    ExpandedPathBuf::with_context(input, None, lookup)
}

fn home(rest: &str) -> PathBuf {
    Path::new(HOME).join(rest)
}

fn unset(name: &str) -> ExpandError {
    ExpandError::UnsetVariable {
        name: name.to_owned(),
    }
}

#[test]
fn expands_leading_tilde() {
    assert_eq!(expand("~").unwrap(), PathBuf::from(HOME));
    assert_eq!(expand("~/x").unwrap(), home("x"));
    assert_eq!(expand("~/a/b.txt").unwrap(), home("a/b.txt"));
}

#[cfg(windows)]
#[test]
fn expands_tilde_before_backslash() {
    assert_eq!(expand(r"~\x").unwrap(), home("x"));
}

#[test]
fn leaves_other_tildes_alone() {
    for input in ["~user/x", "a/~/b", "a~", "~a"] {
        assert_eq!(expand(input).unwrap(), Path::new(input), "{input}");
    }
}

#[test]
fn expands_variables() {
    for (input, want) in [
        ("$VAR", "value"),
        ("$VAR/x", "value/x"),
        ("a/$VAR/b", "a/value/b"),
        ("${VAR}x", "valuex"),
        ("$VAR$VAR", "valuevalue"),
        ("$EMPTY/x", "/x"),
        ("~/$VAR", "/home/test/value"),
        ("~$VAR", "~value"),
    ] {
        assert_eq!(expand(input).unwrap(), Path::new(want), "{input}");
    }
}

#[test]
fn uses_default_for_unset_variable() {
    assert_eq!(
        expand("${UNSET:-fallback}/x").unwrap(),
        Path::new("fallback/x")
    );
}

#[test]
fn set_but_empty_variable_is_not_replaced_by_default() {
    assert_eq!(expand("${EMPTY:-fallback}").unwrap(), Path::new(""));
}

#[test]
fn values_and_defaults_are_inserted_verbatim() {
    for (input, want) in [
        ("$TILDE", "~/from-var"),
        ("$DOLLAR", "$VAR"),
        ("${UNSET:-$VAR}", "$VAR"),
        ("${UNSET:-~/x}", "~/x"),
    ] {
        assert_eq!(expand(input).unwrap(), Path::new(want), "{input}");
    }
}

#[test]
fn dollar_escapes() {
    for (input, want) in [
        ("a$$b", "a$b"),
        ("$$VAR", "$VAR"),
        ("a$.b", "a$.b"),
        ("100$", "100$"),
    ] {
        assert_eq!(expand(input).unwrap(), Path::new(want), "{input}");
    }
}

#[test]
fn unset_variable_is_an_error() {
    for input in ["$UNSET", "${UNSET}", "a/$UNSET/b"] {
        assert_eq!(expand(input), Err(unset("UNSET")), "{input}");
    }
}

#[test]
fn tilde_without_home_directory_is_an_error() {
    assert_eq!(
        expand_without_home("~"),
        Err(ExpandError::HomeDirUnavailable)
    );
    assert_eq!(
        expand_without_home("~/x"),
        Err(ExpandError::HomeDirUnavailable)
    );
}

#[test]
fn home_directory_is_only_needed_for_a_leading_tilde() {
    assert_eq!(expand_without_home("x/y").unwrap(), Path::new("x/y"));
    assert_eq!(
        expand_without_home("~user/x").unwrap(),
        Path::new("~user/x")
    );
    assert_eq!(expand_without_home("$VAR").unwrap(), Path::new("value"));
}

#[test]
fn relative_input_stays_relative() {
    let path = expand("foo/bar").unwrap();
    assert_eq!(path, Path::new("foo/bar"));
    assert!(path.is_relative());
}

#[cfg(unix)]
#[test]
fn non_utf8_input_and_values_are_expanded() {
    use std::os::unix::ffi::OsStrExt;

    let input = OsStr::from_bytes(b"~/\xff");
    assert_eq!(
        expand(input).unwrap(),
        Path::new(HOME).join(OsStr::from_bytes(b"\xff"))
    );

    let value = OsStr::from_bytes(b"/tmp/\xff");
    let path = ExpandedPathBuf::with_context("$DIR/x", Some(Path::new(HOME)), |name| {
        (name == "DIR").then_some(value)
    })
    .unwrap();
    assert_eq!(path, Path::new(OsStr::from_bytes(b"/tmp/\xff/x")));
}

#[test]
fn process_environment_is_the_default_context() {
    let path = std::env::var_os("PATH").expect("PATH is always set");
    assert_eq!(ExpandedPathBuf::new("$PATH").unwrap(), Path::new(&path));
    assert_eq!(
        ExpandedPathBuf::new("$__EXPANDED_PATHBUF_TEST_UNSET__"),
        Err(unset("__EXPANDED_PATHBUF_TEST_UNSET__"))
    );
    match dirs::home_dir() {
        Some(home) => assert_eq!(ExpandedPathBuf::new("~").unwrap(), home),
        None => assert_eq!(
            ExpandedPathBuf::new("~"),
            Err(ExpandError::HomeDirUnavailable)
        ),
    }
}

#[test]
fn every_conversion_expands() {
    // Only a constructor that expands can notice an unset variable.
    let input = "$__EXPANDED_PATHBUF_TEST_UNSET__";
    let want = Err(unset("__EXPANDED_PATHBUF_TEST_UNSET__"));
    assert_eq!(input.parse::<ExpandedPathBuf>(), want);
    assert_eq!(ExpandedPathBuf::try_from(input), want);
    assert_eq!(ExpandedPathBuf::try_from(String::from(input)), want);
    assert_eq!(ExpandedPathBuf::try_from(OsStr::new(input)), want);
    assert_eq!(ExpandedPathBuf::try_from(OsString::from(input)), want);
    assert_eq!(ExpandedPathBuf::try_from(Path::new(input)), want);
    assert_eq!(ExpandedPathBuf::try_from(PathBuf::from(input)), want);
}

#[test]
fn derefs_to_path_and_converts_back_to_path_buf() {
    let path = expand("~/dir/file.txt").unwrap();
    assert_eq!(path.file_name(), Some(OsStr::new("file.txt")));
    assert_eq!(path.as_path(), home("dir/file.txt").as_path());

    fn takes_path(path: &Path) -> &Path {
        path
    }
    assert_eq!(takes_path(&path), home("dir/file.txt"));
    fn takes_as_ref(path: impl AsRef<Path>) -> PathBuf {
        path.as_ref().to_path_buf()
    }
    assert_eq!(takes_as_ref(&path), home("dir/file.txt"));
    let os_str: &OsStr = path.as_ref();
    assert_eq!(os_str, home("dir/file.txt").as_os_str());

    assert_eq!(path.clone().into_path_buf(), home("dir/file.txt"));
    assert_eq!(PathBuf::from(path), home("dir/file.txt"));
}

#[test]
fn compares_and_orders_against_std_path_types() {
    let x = expand("~/x").unwrap();
    let y = expand("~/y").unwrap();

    assert_eq!(x, home("x"));
    assert_eq!(home("x"), x);
    assert_eq!(x, *home("x").as_path());
    assert_eq!(*home("x").as_path(), x);
    assert_eq!(x, home("x").as_path());
    assert_eq!(home("x").as_path(), x);
    assert_ne!(x, home("y"));

    assert!(x < y);
    assert!(x < home("y"));
    assert!(home("y") > x);
    assert!(x < *home("y").as_path());
    assert!(x < home("y").as_path());
}

#[test]
fn is_a_map_key_that_can_be_looked_up_by_path() {
    let map: HashMap<ExpandedPathBuf, u32> = HashMap::from([(expand("~/x").unwrap(), 1)]);
    assert_eq!(map.get(home("x").as_path()), Some(&1));

    let set: HashSet<ExpandedPathBuf> = HashSet::from([expand("~/x").unwrap()]);
    assert!(set.contains(home("x").as_path()));
    assert!(!set.contains(home("y").as_path()));
}

#[test]
fn default_is_the_empty_path() {
    #[derive(Default)]
    struct Config {
        path: ExpandedPathBuf,
    }
    assert_eq!(Config::default().path, Path::new(""));
}

#[test]
fn error_is_a_std_error_naming_the_variable() {
    fn assert_error<E: std::error::Error + Clone + PartialEq + Send + Sync + 'static>(_: &E) {}
    let err = expand("$UNSET").unwrap_err();
    assert_error(&err);
    assert!(err.to_string().contains("UNSET"), "{err}");
}
