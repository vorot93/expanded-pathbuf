#![doc = include_str!("../README.md")]

use derive_more::*;
use std::{path::PathBuf, str::FromStr};

#[derive(AsRef, Clone, Debug, Deref, DerefMut, PartialEq, Eq, From, Hash, PartialOrd, Ord)]
#[as_ref(forward)]
#[from(forward)]
pub struct ExpandedPathBuf(pub PathBuf);

impl FromStr for ExpandedPathBuf {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(PathBuf::from_str(&shellexpand::full(s)?)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand() {
        let path = "~/hello";

        assert_eq!(
            ExpandedPathBuf::from_str(path).unwrap().0,
            directories::BaseDirs::new()
                .unwrap()
                .home_dir()
                .join("hello")
        );
    }
}
