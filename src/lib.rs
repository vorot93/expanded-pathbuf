#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

use derive_more::{AsRef, Deref};
use std::{
    borrow::{Borrow, Cow},
    cmp::Ordering,
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    str::FromStr,
};

/// The reason an input could not be expanded.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ExpandError {
    /// The input starts with `~` (alone or followed by a separator), but no home directory is
    /// available to replace it with.
    #[error("cannot expand `~`: no home directory is available")]
    HomeDirUnavailable,
    /// The input references a variable that is not set and has no `${NAME:-default}` fallback.
    #[error("cannot expand `${name}`: the variable is not set")]
    UnsetVariable {
        /// The name of the variable, without the `$` or braces.
        name: String,
    },
}

/// A [`PathBuf`] whose `~` and variable references were expanded when it was constructed.
///
/// Every constructor — [`new`](Self::new), [`with_context`](Self::with_context), [`FromStr`] and
/// the [`TryFrom`] impls for the standard string and path types — performs expansion, and the
/// value cannot be modified afterwards, so holding an `ExpandedPathBuf` is proof that expansion
/// has happened. The expansion rules are documented at the [crate level](crate).
///
/// The type derefs to [`Path`], so read-only path operations are available directly; use
/// [`into_path_buf`](Self::into_path_buf) or `PathBuf::from` to get a mutable [`PathBuf`] back.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, AsRef, Deref)]
#[as_ref(forward)]
#[deref(forward)]
pub struct ExpandedPathBuf(PathBuf);

impl ExpandedPathBuf {
    /// Expands `input` using the process environment ([`std::env::var_os`]) and the current
    /// user's home directory ([`dirs::home_dir`]).
    ///
    /// # Errors
    ///
    /// See [`ExpandError`].
    pub fn new(input: impl AsRef<Path>) -> Result<Self, ExpandError> {
        Self::expand(input.as_ref(), dirs::home_dir, |name| {
            std::env::var_os(name)
        })
    }

    /// Expands `input` using a caller-supplied home directory and variable lookup instead of the
    /// process environment.
    ///
    /// `lookup` receives a variable name (without the `$` or braces) and returns its value, or
    /// `None` if the variable is not set. A leading `~` expands to `home_dir`; with `None`, such
    /// inputs fail with [`ExpandError::HomeDirUnavailable`].
    ///
    /// # Errors
    ///
    /// See [`ExpandError`].
    pub fn with_context<V: AsRef<Path>>(
        input: impl AsRef<Path>,
        home_dir: Option<&Path>,
        lookup: impl FnMut(&str) -> Option<V>,
    ) -> Result<Self, ExpandError> {
        Self::expand(input.as_ref(), || home_dir, lookup)
    }

    fn expand<H: AsRef<Path>, V: AsRef<Path>>(
        input: &Path,
        home_dir: impl FnOnce() -> Option<H>,
        mut lookup: impl FnMut(&str) -> Option<V>,
    ) -> Result<Self, ExpandError> {
        // shellexpand asks for the home directory only while expanding a leading `~`, and leaves
        // the `~` in place when the answer is `None`. Remember that answer so it becomes an error
        // rather than a path with a literal `~` component.
        let mut home_dir_missing = false;
        let home_dir = || {
            let home_dir = home_dir();
            home_dir_missing = home_dir.is_none();
            home_dir
        };
        // `Ok(None)` would make shellexpand leave `$NAME` in place; an unset variable is an error.
        let lookup = |name: &str| match lookup(name) {
            Some(value) => Ok(Some(value)),
            None => Err(ExpandError::UnsetVariable {
                name: name.to_owned(),
            }),
        };
        let expanded = shellexpand::path::full_with_context(input, home_dir, lookup)
            .map_err(|error| error.cause)?;
        if home_dir_missing {
            return Err(ExpandError::HomeDirUnavailable);
        }
        Ok(Self(expanded.into_owned()))
    }

    /// Borrows the expanded path.
    pub fn as_path(&self) -> &Path {
        &self.0
    }

    /// Unwraps the expanded path into a plain, mutable [`PathBuf`].
    pub fn into_path_buf(self) -> PathBuf {
        self.0
    }
}

impl FromStr for ExpandedPathBuf {
    type Err = ExpandError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Self::new(input)
    }
}

macro_rules! impl_try_from {
    ($($input:ty),* $(,)?) => {$(
        impl TryFrom<$input> for ExpandedPathBuf {
            type Error = ExpandError;

            fn try_from(input: $input) -> Result<Self, Self::Error> {
                Self::new(input)
            }
        }
    )*};
}

impl_try_from!(&str, String, &OsStr, OsString, &Path, PathBuf);

impl From<ExpandedPathBuf> for PathBuf {
    fn from(path: ExpandedPathBuf) -> Self {
        path.0
    }
}

impl Borrow<Path> for ExpandedPathBuf {
    fn borrow(&self) -> &Path {
        &self.0
    }
}

macro_rules! impl_cmp {
    ($($other:ty),* $(,)?) => {$(
        impl PartialEq<$other> for ExpandedPathBuf {
            fn eq(&self, other: &$other) -> bool {
                self.as_path() == AsRef::<Path>::as_ref(other)
            }
        }

        impl PartialEq<ExpandedPathBuf> for $other {
            fn eq(&self, other: &ExpandedPathBuf) -> bool {
                AsRef::<Path>::as_ref(self) == other.as_path()
            }
        }

        impl PartialOrd<$other> for ExpandedPathBuf {
            fn partial_cmp(&self, other: &$other) -> Option<Ordering> {
                self.as_path().partial_cmp(AsRef::<Path>::as_ref(other))
            }
        }

        impl PartialOrd<ExpandedPathBuf> for $other {
            fn partial_cmp(&self, other: &ExpandedPathBuf) -> Option<Ordering> {
                AsRef::<Path>::as_ref(self).partial_cmp(other.as_path())
            }
        }
    )*};
}

impl_cmp!(Path, &Path, PathBuf, Cow<'_, Path>);

/// Constructors and mutators that would bypass expansion must not exist.
///
/// clap's `value_parser!` prefers `From<OsString>`/`From<&OsStr>`/`From<String>`/`From<&str>`
/// over `FromStr`, and `Into` hands out unexpanded values, so no `From` impl is allowed:
///
/// ```compile_fail,E0277
/// let _: expanded_pathbuf::ExpandedPathBuf = "~/x".into();
/// ```
/// ```compile_fail,E0277
/// let _: expanded_pathbuf::ExpandedPathBuf = std::ffi::OsString::from("~/x").into();
/// ```
/// ```compile_fail,E0277
/// let _: expanded_pathbuf::ExpandedPathBuf = std::path::PathBuf::from("~/x").into();
/// ```
///
/// The inner path is private and cannot be mutated after construction:
///
/// ```compile_fail,E0603
/// let _ = expanded_pathbuf::ExpandedPathBuf(std::path::PathBuf::from("~/x"));
/// ```
/// ```compile_fail,E0599
/// let mut path: expanded_pathbuf::ExpandedPathBuf = "/x".parse().unwrap();
/// path.push("~/y");
/// ```
#[cfg(doctest)]
struct NoBypasses;
