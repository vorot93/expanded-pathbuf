# expanded-pathbuf
This library is a wrapper around `PathBuf` that automatically expands environment variables and tilde (`~`) in paths when constructing from a string.

## Features
- Converts strings with `~` and environment variables into absolute paths.
- Transparently derefs into `PathBuf`.

## Example
```rust
use expanded_pathbuf::ExpandedPathBuf;
use std::str::FromStr;

let path = ExpandedPathBuf::from_str("~/file.txt").unwrap();
println!("{}", path.display());
```

## License
The entire code within this repository is licensed under the [Mozilla Public License v2.0](./LICENSE)
