# expanded-pathbuf

A `PathBuf` that expands `~` and environment variables when it is constructed.

`ExpandedPathBuf` wraps `std::path::PathBuf`. Every way to construct one — `ExpandedPathBuf::new`,
`str::parse` (`FromStr`), and `TryFrom` for the standard string and path types — runs the input
through expansion, and the value cannot be modified afterwards, so holding an `ExpandedPathBuf`
means expansion has happened. It is meant for paths that come from configuration files and
command-line arguments, and it works with clap's derive API through `FromStr`.

## Examples

```rust
use expanded_pathbuf::ExpandedPathBuf;

let config: ExpandedPathBuf = "~/.config/app/config.toml".parse()?;
println!("{}", config.display());
# Ok::<(), expanded_pathbuf::ExpandError>(())
```

With clap, the argument is expanded while it is parsed:

```rust
use clap::Parser;
use expanded_pathbuf::ExpandedPathBuf;

#[derive(Parser)]
struct Cli {
    /// `~` and `$VAR` in this argument are expanded.
    #[arg(long)]
    output: ExpandedPathBuf,
}
```

The home directory and the variables can come from the caller instead of the process — for tests,
or for configuration formats that define their own variables:

```rust
use expanded_pathbuf::ExpandedPathBuf;
use std::path::Path;

let path = ExpandedPathBuf::with_context(
    "~/$PROJECT/build",
    Some(Path::new("/home/me")),
    |name| (name == "PROJECT").then_some("app"),
)?;
assert_eq!(path, Path::new("/home/me/app/build"));
# Ok::<(), expanded_pathbuf::ExpandError>(())
```

## Expansion rules

Input is processed as bytes (`OsStr`), so non-UTF-8 paths and variable values work. Variables
are expanded first, then the tilde.

| Input | Result |
|---|---|
| `~`, `~/rest` (also `~\rest` on Windows) | the home directory, then the rest of the input |
| `$NAME`, `${NAME}` | the value of the variable `NAME` |
| `${NAME:-default}` | the value of `NAME`, or `default` if `NAME` is not set |
| `$$` | a literal `$` |
| `$` followed by anything else (`$.`, `$-`, `$ `, end of input) | a literal `$` |

`NAME` is a run of letters, digits and `_`; the `${...}` form accepts any characters up to the
closing brace.

Things that are **not** expanded, on any platform:

- `~user/rest` is left as is.
- The Windows `%NAME%` syntax is left as is.
- A `~` or `$` inside a variable's value or inside a `${NAME:-default}` default is inserted
  verbatim and never re-expanded. `${XDG_CONFIG_HOME:-$HOME/.config}` therefore yields the literal
  `$HOME/.config` when `XDG_CONFIG_HOME` is unset — spell the fallback out, or resolve it in code.
- Anything else. Relative paths stay relative; nothing is canonicalized or checked against the
  filesystem.

Two situations are errors (`ExpandError`) rather than silent passthrough, because a literal `~`
or `$NAME` left in a path usually ends up as a directory named `~` or `$NAME`:

- `$NAME`/`${NAME}` where `NAME` is not set: `ExpandError::UnsetVariable`. A variable that is set
  to the empty string counts as set, so `${NAME:-default}` yields the empty string rather than
  `default` (unlike in POSIX shells).
- `~`/`~/rest` when no home directory can be determined: `ExpandError::HomeDirUnavailable`.

By default the process environment (`std::env::var_os`) and the current user's home directory
(`dirs::home_dir`) are consulted; `ExpandedPathBuf::with_context` takes both from the caller.

## Using the value

`ExpandedPathBuf` derefs to `Path` and implements `AsRef<Path>`, `AsRef<OsStr>` and
`Borrow<Path>`, so it can be passed wherever a path is expected and used as a map key that is
looked up by `&Path`. It compares and orders directly against `Path`, `&Path`, `PathBuf` and
`Cow<Path>`, and `Default` is the empty path.

It cannot be modified in place — there is no `DerefMut`, `push` or `set_file_name` — because
expansion is a property of the whole value. Use `into_path_buf` or `PathBuf::from` to get a plain
`PathBuf` when you need to build on it.

## License

Licensed under the [Apache License, Version 2.0](https://github.com/vorot93/expanded-pathbuf/blob/master/LICENSE).
