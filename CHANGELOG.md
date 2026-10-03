# Changelog

## 0.1.1 - 2026-10-03

- Windows: `symlink_auto` converts forward slashes to backslashes in target paths
  before creating links, fixing failures with relative targets such as `t/a`.
- Windows: `symlink_auto` strips trailing separators from directory targets before
  creating links, fixing failures with targets such as `sub/`.

## 0.1.0 - 2026-08-22

### Added

- Six functions for creating and removing symbolic links on Unix and Windows.
- Refusal checks that preserve regular files and directories.
- Windows link-kind detection for explicit and automatic removal.
- Parent-relative target probing for automatic link creation on Windows.
- Unsupported-target errors for the complete public API.
