# portable-symlink

`portable-symlink` creates and removes symbolic links on Unix and Windows. Its six functions use the Rust standard library and add no package dependencies.

Removal functions inspect the path with `symlink_metadata` before deleting it. A regular file or directory returns `InvalidInput` and stays in place. Removal affects the link. The target and its contents stay intact.

## Usage

```rust
use portable_symlink::{remove_symlink_auto, symlink_auto};

# fn run() -> std::io::Result<()> {
symlink_auto("records/current.log", "latest.log")?;
remove_symlink_auto("latest.log")?;
# Ok(())
# }
```

The creation functions take `(target, link)` in that order:

- `symlink_file` creates a file-kind link on Windows.
- `symlink_dir` creates a directory-kind link on Windows.
- `symlink_auto` probes the target on Windows and selects its kind.

Unix links do not carry a file or directory kind, so the three creation functions call the same operating system function there.

## Removal

- `remove_symlink_file` removes a file-kind link.
- `remove_symlink_dir` removes a directory-kind link or Windows junction.
- `remove_symlink_auto` reads the Windows kind and selects the matching removal function.

On Windows, passing the wrong kind to an explicit removal function returns `InvalidInput`. The automatic removal function handles either kind. A process still needs the Windows symbolic-link privilege or Developer Mode to create links.

## Relation to the symlink crate

This crate is an independent implementation. It is not affiliated with the author of the `symlink` crate. It keeps the same six function names and generic argument forms. Three behavior changes protect callers and fix Windows path handling:

1. Every removal function refuses a regular file or directory.
2. Windows `symlink_auto` resolves a relative target from the link's parent directory. A missing target returns the probe error.
3. Windows `remove_symlink_auto` removes file-kind and directory-kind links, including dangling links, after inspecting the link itself.

Targets outside Unix and Windows expose all six functions and return `Unsupported`.

## Rust version

The minimum supported Rust version is 1.63.0.

## License

MIT
