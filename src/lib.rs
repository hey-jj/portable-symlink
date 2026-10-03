#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::io;
use std::path::Path;

/// Creates a file-kind symbolic link at `link` that points to `target`.
///
/// Unix symbolic links do not carry a file or directory kind. On Unix, this
/// function has the same behavior as [`symlink_dir`] and [`symlink_auto`].
pub fn symlink_file<P: AsRef<Path>, Q: AsRef<Path>>(target: P, link: Q) -> io::Result<()> {
    platform::symlink_file(target.as_ref(), link.as_ref())
}

/// Creates a directory-kind symbolic link at `link` that points to `target`.
///
/// Unix symbolic links do not carry a file or directory kind. On Unix, this
/// function has the same behavior as [`symlink_file`] and [`symlink_auto`].
pub fn symlink_dir<P: AsRef<Path>, Q: AsRef<Path>>(target: P, link: Q) -> io::Result<()> {
    platform::symlink_dir(target.as_ref(), link.as_ref())
}

/// Creates a symbolic link and selects its Windows kind from the target.
///
/// On Windows, a relative target is resolved against the parent of `link` for
/// the kind probe. A missing target returns the probe error. Unix creates the
/// link without reading the target.
pub fn symlink_auto<P: AsRef<Path>, Q: AsRef<Path>>(target: P, link: Q) -> io::Result<()> {
    platform::symlink_auto(target.as_ref(), link.as_ref())
}

/// Removes a file-kind symbolic link.
///
/// A path that is not a symbolic link is refused with
/// [`io::ErrorKind::InvalidInput`]. Unix symbolic links do not carry a kind.
pub fn remove_symlink_file<P: AsRef<Path>>(link: P) -> io::Result<()> {
    platform::remove_symlink_file(link.as_ref())
}

/// Removes a directory-kind symbolic link or Windows junction.
///
/// A path that is not a symbolic link is refused with
/// [`io::ErrorKind::InvalidInput`]. Unix symbolic links do not carry a kind.
pub fn remove_symlink_dir<P: AsRef<Path>>(link: P) -> io::Result<()> {
    platform::remove_symlink_dir(link.as_ref())
}

/// Removes a symbolic link after reading its Windows kind.
///
/// A path that is not a symbolic link is refused with
/// [`io::ErrorKind::InvalidInput`]. The target is not followed or removed.
pub fn remove_symlink_auto<P: AsRef<Path>>(link: P) -> io::Result<()> {
    platform::remove_symlink_auto(link.as_ref())
}

#[cfg(unix)]
mod platform {
    use std::fs;
    use std::io;
    use std::os::unix::fs::symlink;
    use std::path::Path;

    pub(super) fn symlink_file(target: &Path, link: &Path) -> io::Result<()> {
        symlink(target, link)
    }

    pub(super) fn symlink_dir(target: &Path, link: &Path) -> io::Result<()> {
        symlink(target, link)
    }

    pub(super) fn symlink_auto(target: &Path, link: &Path) -> io::Result<()> {
        symlink(target, link)
    }

    pub(super) fn remove_symlink_file(link: &Path) -> io::Result<()> {
        refuse_non_symlink(link)?;
        fs::remove_file(link)
    }

    pub(super) fn remove_symlink_dir(link: &Path) -> io::Result<()> {
        refuse_non_symlink(link)?;
        fs::remove_file(link)
    }

    pub(super) fn remove_symlink_auto(link: &Path) -> io::Result<()> {
        refuse_non_symlink(link)?;
        fs::remove_file(link)
    }

    fn refuse_non_symlink(link: &Path) -> io::Result<fs::Metadata> {
        let metadata = fs::symlink_metadata(link)?;
        if metadata.file_type().is_symlink() {
            Ok(metadata)
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("not a symbolic link: {}", link.display()),
            ))
        }
    }
}

#[cfg(windows)]
mod platform {
    use std::ffi::OsString;
    use std::fs;
    use std::io;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use std::os::windows::fs::MetadataExt;
    use std::os::windows::fs::{symlink_dir as create_dir_link, symlink_file as create_file_link};
    use std::path::{Path, PathBuf};

    // FILE_ATTRIBUTE_DIRECTORY from the Win32 file attribute constants.
    const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;

    pub(super) fn symlink_file(target: &Path, link: &Path) -> io::Result<()> {
        create_file_link(target, link)
    }

    pub(super) fn symlink_dir(target: &Path, link: &Path) -> io::Result<()> {
        create_dir_link(target, link)
    }

    pub(super) fn symlink_auto(target: &Path, link: &Path) -> io::Result<()> {
        let target_is_dir = if target.is_absolute() {
            fs::metadata(target)?.is_dir()
        } else {
            let probe = link.parent().unwrap_or_else(|| Path::new("")).join(target);
            fs::metadata(probe)?.is_dir()
        };

        let win_target = win_normalise_auto_target(target, target_is_dir);
        if target_is_dir {
            create_dir_link(&win_target, link)
        } else {
            create_file_link(&win_target, link)
        }
    }

    fn win_normalise_auto_target(target: &Path, is_dir: bool) -> PathBuf {
        let mut target: Vec<u16> = target
            .as_os_str()
            .encode_wide()
            .map(|c| {
                if c == u16::from(b'/') {
                    u16::from(b'\\')
                } else {
                    c
                }
            })
            .collect();
        if is_dir {
            while target.last() == Some(&u16::from(b'\\')) {
                target.pop();
            }
        }
        PathBuf::from(OsString::from_wide(&target))
    }

    pub(super) fn remove_symlink_file(link: &Path) -> io::Result<()> {
        let metadata = refuse_non_symlink(link)?;
        if metadata.file_attributes() & FILE_ATTRIBUTE_DIRECTORY != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("not a file-kind symbolic link: {}", link.display()),
            ));
        }
        fs::remove_file(link)
    }

    pub(super) fn remove_symlink_dir(link: &Path) -> io::Result<()> {
        let metadata = refuse_non_symlink(link)?;
        if metadata.file_attributes() & FILE_ATTRIBUTE_DIRECTORY == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("not a directory-kind symbolic link: {}", link.display()),
            ));
        }
        fs::remove_dir(link)
    }

    pub(super) fn remove_symlink_auto(link: &Path) -> io::Result<()> {
        let metadata = refuse_non_symlink(link)?;
        if metadata.file_attributes() & FILE_ATTRIBUTE_DIRECTORY != 0 {
            fs::remove_dir(link)
        } else {
            fs::remove_file(link)
        }
    }

    fn refuse_non_symlink(link: &Path) -> io::Result<fs::Metadata> {
        let metadata = fs::symlink_metadata(link)?;
        if metadata.file_type().is_symlink() {
            Ok(metadata)
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("not a symbolic link: {}", link.display()),
            ))
        }
    }
}

#[cfg(not(any(unix, windows)))]
mod platform {
    use std::io;
    use std::path::Path;

    const MESSAGE: &str = "symbolic links are not supported on this target";

    pub(super) fn symlink_file(_target: &Path, _link: &Path) -> io::Result<()> {
        unsupported()
    }

    pub(super) fn symlink_dir(_target: &Path, _link: &Path) -> io::Result<()> {
        unsupported()
    }

    pub(super) fn symlink_auto(_target: &Path, _link: &Path) -> io::Result<()> {
        unsupported()
    }

    pub(super) fn remove_symlink_file(_link: &Path) -> io::Result<()> {
        unsupported()
    }

    pub(super) fn remove_symlink_dir(_link: &Path) -> io::Result<()> {
        unsupported()
    }

    pub(super) fn remove_symlink_auto(_link: &Path) -> io::Result<()> {
        unsupported()
    }

    fn unsupported() -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Unsupported, MESSAGE))
    }
}

#[cfg(test)]
mod unit_tests;
