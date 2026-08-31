use super::*;

#[test]
fn public_functions_accept_distinct_path_types() {
    fn check_create<P: AsRef<Path>, Q: AsRef<Path>>(_function: fn(P, Q) -> io::Result<()>) {}

    fn check_remove<P: AsRef<Path>>(_function: fn(P) -> io::Result<()>) {}

    check_create::<&str, &Path>(symlink_file);
    check_create::<&str, &Path>(symlink_dir);
    check_create::<&str, &Path>(symlink_auto);
    check_remove::<&Path>(remove_symlink_file);
    check_remove::<&Path>(remove_symlink_dir);
    check_remove::<&Path>(remove_symlink_auto);
}

#[test]
fn crate_forbids_unsafe_code() {
    assert!(include_str!("lib.rs").contains("#![forbid(unsafe_code)]"));
}

#[cfg(not(any(unix, windows)))]
#[test]
fn unsupported_target_returns_unsupported() {
    assert_eq!(
        symlink_file("target", "link")
            .expect_err("unsupported target must return an error")
            .kind(),
        io::ErrorKind::Unsupported
    );
    assert_eq!(
        symlink_dir("target", "link")
            .expect_err("unsupported target must return an error")
            .kind(),
        io::ErrorKind::Unsupported
    );
    assert_eq!(
        symlink_auto("target", "link")
            .expect_err("unsupported target must return an error")
            .kind(),
        io::ErrorKind::Unsupported
    );
    assert_eq!(
        remove_symlink_file("link")
            .expect_err("unsupported target must return an error")
            .kind(),
        io::ErrorKind::Unsupported
    );
    assert_eq!(
        remove_symlink_dir("link")
            .expect_err("unsupported target must return an error")
            .kind(),
        io::ErrorKind::Unsupported
    );
    assert_eq!(
        remove_symlink_auto("link")
            .expect_err("unsupported target must return an error")
            .kind(),
        io::ErrorKind::Unsupported
    );
}
