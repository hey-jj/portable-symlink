use portable_symlink::{
    remove_symlink_auto, remove_symlink_dir, remove_symlink_file, symlink_auto, symlink_dir,
    symlink_file,
};
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

static NEXT_TREE: AtomicU64 = AtomicU64::new(0);
static CURRENT_DIR_LOCK: Mutex<()> = Mutex::new(());

struct TempTree {
    root: PathBuf,
}

impl TempTree {
    fn new(label: &str) -> Self {
        let number = NEXT_TREE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "portable-symlink-{}-{}-{number}",
            std::process::id(),
            label
        ));
        fs::create_dir(&root).expect("create temporary test directory");
        Self { root }
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    fn dir(&self, relative: &str) -> PathBuf {
        let path = self.path(relative);
        fs::create_dir_all(&path).expect("create test directory");
        path
    }

    fn file(&self, relative: &str, contents: &[u8]) -> PathBuf {
        let path = self.path(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create file parent");
        }
        fs::write(&path, contents).expect("write test file");
        path
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

struct CurrentDir {
    original: PathBuf,
    _lock: MutexGuard<'static, ()>,
}

impl CurrentDir {
    fn enter(path: &Path) -> Self {
        let lock = CURRENT_DIR_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let original = std::env::current_dir().expect("read current directory");
        std::env::set_current_dir(path).expect("change current directory");
        Self {
            original,
            _lock: lock,
        }
    }
}

impl Drop for CurrentDir {
    fn drop(&mut self) {
        std::env::set_current_dir(&self.original).expect("restore current directory");
    }
}

fn assert_link(path: &Path) {
    let metadata = fs::symlink_metadata(path).expect("read link metadata");
    assert!(metadata.file_type().is_symlink());
}

fn assert_refused(result: io::Result<()>, path: &Path) {
    let error = result.expect_err("non-link path must be refused");
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    assert_eq!(error.raw_os_error(), None);
    assert!(error.to_string().contains(&path.display().to_string()));
}

fn assert_no_panic<F>(call: F)
where
    F: FnOnce() + std::panic::UnwindSafe,
{
    assert!(std::panic::catch_unwind(call).is_ok());
}

#[cfg(windows)]
fn is_directory_kind(path: &Path) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
    fs::symlink_metadata(path)
        .expect("read Windows link metadata")
        .file_attributes()
        & FILE_ATTRIBUTE_DIRECTORY
        != 0
}

#[cfg(windows)]
fn create_junction(target: &Path, junction: &Path) {
    use std::process::Command;

    let status = Command::new("cmd")
        .arg("/C")
        .arg("mklink")
        .arg("/J")
        .arg(junction)
        .arg(target)
        .status()
        .expect("run mklink");
    assert!(status.success());
}

#[test]
fn conformance_fixture_matches_exact_scenario_set() {
    let fixture = include_str!("data/conformance-scenarios.txt");
    let source = include_str!("scenarios.rs");
    let mut expected = BTreeSet::new();
    let mut manual = BTreeSet::new();

    for (index, line) in fixture.lines().enumerate() {
        let mut fields = line.split_ascii_whitespace();
        let status = fields.next().expect("scenario status");
        let id = fields.next().expect("scenario id");
        assert!(
            fields.next().is_none(),
            "extra field on fixture line {}",
            index + 1
        );

        let name = format!("scenario_{}", id.to_ascii_lowercase().replace('-', "_"));
        match status {
            "test" => assert!(expected.insert(name), "duplicate test scenario {id}"),
            "manual" => assert!(manual.insert(name), "duplicate manual scenario {id}"),
            _ => panic!("unknown status on fixture line {}", index + 1),
        };
    }

    let actual = source
        .lines()
        .filter_map(|line| {
            line.trim_start()
                .strip_prefix("fn ")?
                .strip_suffix("() {")?
                .strip_prefix("scenario_")
                .map(|suffix| format!("scenario_{suffix}"))
        })
        .collect::<BTreeSet<_>>();

    assert_eq!(actual, expected);
    assert_eq!(
        manual,
        BTreeSet::from([
            String::from("scenario_cr_14"),
            String::from("scenario_rf_08"),
        ])
    );
}

#[test]
fn scenario_cr_01() {
    let tree = TempTree::new("cr01");
    tree.file("a", b"alpha");
    let link = tree.path("la");

    symlink_file("a", &link).expect("create file link");

    assert_link(&link);
    assert_eq!(fs::read_link(&link).expect("read link"), Path::new("a"));
    assert_eq!(fs::read(&link).expect("read through link"), b"alpha");
}

#[test]
fn scenario_cr_02() {
    let tree = TempTree::new("cr02");
    tree.file("sub/x", b"x");
    let link = tree.path("ld");

    symlink_dir("sub", &link).expect("create directory link");

    assert_link(&link);
    assert_eq!(fs::read(link.join("x")).expect("read linked file"), b"x");
}

#[test]
fn scenario_cr_03() {
    let tree = TempTree::new("cr03");
    let target = tree.file("a", b"a");
    let link = tree.path("la");

    symlink_file(&target, &link).expect("create absolute link");

    assert_eq!(fs::read_link(link).expect("read link"), target);
}

#[test]
fn scenario_cr_04() {
    let tree = TempTree::new("cr04");
    let link = tree.path("la");

    symlink_file("missing", &link).expect("create dangling file link");

    assert_link(&link);
    assert_eq!(
        fs::metadata(link)
            .expect_err("dangling link must not resolve")
            .kind(),
        io::ErrorKind::NotFound
    );
}

#[test]
fn scenario_cr_05() {
    let tree = TempTree::new("cr05");
    let link = tree.path("ld");

    symlink_dir("missing", &link).expect("create dangling directory link");

    assert_link(&link);
    #[cfg(windows)]
    assert!(is_directory_kind(&link));
}

#[test]
fn scenario_cr_06() {
    let tree = TempTree::new("cr06");
    let occupied = tree.file("exists", b"keep");

    let error = symlink_file("x", &occupied).expect_err("occupied path must fail");

    assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
    assert_eq!(fs::read(occupied).expect("read preserved file"), b"keep");
}

#[test]
fn scenario_cr_07() {
    let tree = TempTree::new("cr07");
    let link = tree.path("no/such/la");

    let error = symlink_file("x", &link).expect_err("missing parent must fail");

    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    assert!(fs::symlink_metadata(link).is_err());
}

#[cfg(unix)]
#[test]
fn scenario_cr_08() {
    let tree = TempTree::new("cr08");
    tree.dir("sub");
    let link = tree.path("l");

    symlink_file("sub/", &link).expect("create link with trailing separator");

    assert_eq!(fs::read_link(link).expect("read link"), Path::new("sub/"));
}

#[test]
fn scenario_cr_09() {
    let tree = TempTree::new("cr09");
    tree.file("a", b"a");
    let link = tree.path("l");

    symlink_dir("a", &link).expect("create directory-kind link to file");

    #[cfg(unix)]
    assert_eq!(fs::read(&link).expect("read typeless link"), b"a");
    #[cfg(windows)]
    {
        assert!(is_directory_kind(&link));
        assert!(fs::File::open(&link).is_err());
        remove_symlink_dir(&link).expect("remove directory-kind link");
    }
}

#[test]
fn scenario_cr_10() {
    let tree = TempTree::new("cr10");
    tree.dir("sub");
    let link = tree.path("l");

    symlink_file("sub", &link).expect("create file-kind link to directory");

    #[cfg(unix)]
    assert!(fs::metadata(&link).expect("follow typeless link").is_dir());
    #[cfg(windows)]
    {
        assert!(!is_directory_kind(&link));
        assert!(fs::read_dir(&link).is_err());
        remove_symlink_file(&link).expect("remove file-kind link");
    }
}

#[test]
fn scenario_cr_11() {
    let tree = TempTree::new("cr11");
    let link = tree.path("l");

    let error = symlink_file("a\0b", &link).expect_err("interior nul must fail");

    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    assert!(fs::symlink_metadata(link).is_err());
}

#[test]
fn scenario_cr_12() {
    let result = std::panic::catch_unwind(|| symlink_file("a", ""));
    assert!(result.is_ok());
    assert!(result.expect("call completed").is_err());
}

#[test]
fn scenario_cr_13() {
    let tree = TempTree::new("cr13");
    let link = tree.path("l");

    let result = std::panic::catch_unwind(|| symlink_file("", &link));
    assert!(result.is_ok());
    if result.expect("call completed").is_ok() {
        assert_link(&link);
    } else {
        assert!(fs::symlink_metadata(link).is_err());
    }
}

// manual_scenario_cr_14: Run on Windows without Developer Mode or link privilege.
// Creation must return raw OS error 1314 and leave the link path absent.

#[test]
fn scenario_au_01() {
    let tree = TempTree::new("au01");
    tree.file("a", b"a");
    let link = tree.path("la");

    symlink_auto("a", &link).expect("create automatic file link");

    assert_link(&link);
    #[cfg(windows)]
    assert!(!is_directory_kind(&link));
}

#[test]
fn scenario_au_02() {
    let tree = TempTree::new("au02");
    tree.file("sub/x", b"x");
    let link = tree.path("ld");

    symlink_auto("sub", &link).expect("create automatic directory link");

    assert_eq!(fs::read(link.join("x")).expect("read linked file"), b"x");
    #[cfg(windows)]
    assert!(is_directory_kind(&link));
}

#[test]
fn scenario_au_03() {
    let tree = TempTree::new("au03");
    let work = tree.dir("w");
    tree.file("t/sub/rel", b"relative");
    let link = tree.path("t/sub/link");
    let _current = CurrentDir::enter(&work);

    symlink_auto("rel", &link).expect("probe relative to link parent");

    assert_eq!(fs::read(link).expect("read relative link"), b"relative");
}

#[test]
fn scenario_au_04() {
    let tree = TempTree::new("au04");
    let work = tree.dir("w");
    tree.dir("w/rel");
    tree.file("t/sub/rel", b"file");
    let link = tree.path("t/sub/link");
    let _current = CurrentDir::enter(&work);

    symlink_auto("rel", &link).expect("probe file beside link");

    assert_eq!(fs::read(&link).expect("read linked file"), b"file");
    #[cfg(windows)]
    assert!(!is_directory_kind(&link));
}

#[test]
fn scenario_au_05() {
    let tree = TempTree::new("au05");
    tree.file("t/a", b"a");
    let _current = CurrentDir::enter(&tree.root);

    symlink_auto("t/a", "la").expect("create bare-name link");

    assert_eq!(fs::read("la").expect("read bare-name link"), b"a");
    #[cfg(windows)]
    assert!(!is_directory_kind(Path::new("la")));
}

#[test]
fn scenario_au_06() {
    let tree = TempTree::new("au06");
    let link = tree.path("l");
    let result = symlink_auto("missing", &link);

    #[cfg(unix)]
    {
        result.expect("Unix creates dangling automatic link");
        assert_link(&link);
    }
    #[cfg(windows)]
    {
        assert_eq!(
            result.expect_err("Windows probes automatic target").kind(),
            io::ErrorKind::NotFound
        );
        assert!(fs::symlink_metadata(link).is_err());
    }
}

#[test]
fn scenario_au_07() {
    let tree = TempTree::new("au07");
    tree.dir("sub");
    let link = tree.path("l");

    symlink_auto("sub/", &link).expect("create link to directory with separator");

    assert!(fs::metadata(&link).expect("follow directory link").is_dir());
    assert_eq!(fs::read_link(link).expect("read link"), Path::new("sub/"));
    #[cfg(windows)]
    assert!(is_directory_kind(&tree.path("l")));
}

#[test]
fn scenario_au_08() {
    let tree = TempTree::new("au08");
    tree.file("a", b"a");
    let link = tree.path("l");
    let result = symlink_auto("a/", &link);

    #[cfg(unix)]
    {
        result.expect("Unix stores trailing separator");
        assert_eq!(fs::read_link(&link).expect("read link"), Path::new("a/"));
        assert!(fs::metadata(link).is_err());
    }
    #[cfg(windows)]
    {
        assert!(result.is_err());
        assert!(fs::symlink_metadata(link).is_err());
    }
}

#[test]
fn scenario_au_09() {
    let tree = TempTree::new("au09");
    let target = tree.file("a", b"absolute");
    tree.dir("dir");
    let link = tree.path("dir/l");

    symlink_auto(&target, &link).expect("create automatic absolute link");

    assert_eq!(fs::read(link).expect("read absolute link"), b"absolute");
}

#[test]
fn scenario_au_10() {
    let tree = TempTree::new("au10");
    tree.file("real/x", b"x");
    let intermediate = tree.path("viadir");
    symlink_dir("real", &intermediate).expect("create intermediate directory link");
    let link = tree.path("l");

    symlink_auto("viadir", &link).expect("probe through intermediate link");

    assert_eq!(fs::read(link.join("x")).expect("read linked file"), b"x");
    #[cfg(windows)]
    assert!(is_directory_kind(&link));
}

#[test]
fn scenario_au_11() {
    let tree = TempTree::new("au11");
    let path = tree.file("a", b"keep");

    let error = symlink_auto(&path, &path).expect_err("occupied path must fail");

    assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
    assert_eq!(fs::read(path).expect("read preserved file"), b"keep");
}

#[test]
fn scenario_rm_01() {
    let tree = TempTree::new("rm01");
    tree.file("a", b"target");
    let link = tree.path("la");
    symlink_file("a", &link).expect("create file link");

    remove_symlink_file(&link).expect("remove file link");

    assert!(fs::symlink_metadata(link).is_err());
    assert_eq!(fs::read(tree.path("a")).expect("read target"), b"target");
}

#[test]
fn scenario_rm_02() {
    let tree = TempTree::new("rm02");
    tree.file("sub/x", b"target");
    let link = tree.path("ld");
    symlink_dir("sub", &link).expect("create directory link");

    remove_symlink_dir(&link).expect("remove directory link");

    assert!(fs::symlink_metadata(link).is_err());
    assert_eq!(
        fs::read(tree.path("sub/x")).expect("read target"),
        b"target"
    );
}

#[test]
fn scenario_rm_03() {
    let tree = TempTree::new("rm03");
    let link = tree.path("dead");
    symlink_file("missing", &link).expect("create dangling file link");

    remove_symlink_auto(&link).expect("remove dangling file link");

    assert!(fs::symlink_metadata(link).is_err());
}

#[test]
fn scenario_rm_04() {
    let tree = TempTree::new("rm04");
    let link = tree.path("dead");
    symlink_dir("missing", &link).expect("create dangling directory link");

    remove_symlink_dir(&link).expect("remove dangling directory link");

    assert!(fs::symlink_metadata(link).is_err());
}

#[test]
fn scenario_rm_05() {
    let tree = TempTree::new("rm05");
    tree.file("a", b"target");
    let link = tree.path("la");
    symlink_file("a", &link).expect("create file link");

    remove_symlink_auto(&link).expect("remove automatic file link");

    assert!(fs::symlink_metadata(link).is_err());
    assert_eq!(fs::read(tree.path("a")).expect("read target"), b"target");
}

#[test]
fn scenario_rm_06() {
    let tree = TempTree::new("rm06");
    tree.file("sub/x", b"target");
    let link = tree.path("ld");
    symlink_dir("sub", &link).expect("create directory link");

    remove_symlink_auto(&link).expect("remove automatic directory link");

    assert!(fs::symlink_metadata(link).is_err());
    assert_eq!(
        fs::read(tree.path("sub/x")).expect("read target"),
        b"target"
    );
}

#[test]
fn scenario_rm_07() {
    let tree = TempTree::new("rm07");
    tree.file("a", b"target");
    let first = tree.path("l1");
    let second = tree.path("l2");
    symlink_file("a", &first).expect("create first link");
    symlink_file("l1", &second).expect("create second link");

    remove_symlink_auto(&second).expect("remove second link");

    assert!(fs::symlink_metadata(second).is_err());
    assert_link(&first);
    assert_eq!(
        fs::read(first).expect("read target through first link"),
        b"target"
    );
}

#[test]
fn scenario_rm_08() {
    let tree = TempTree::new("rm08");
    tree.file("outside/secret", b"secret");
    tree.dir("inside");
    let link = tree.path("inside/ld");
    symlink_dir("../outside", &link).expect("create outside directory link");

    remove_symlink_dir(&link).expect("remove outside directory link");

    assert!(fs::symlink_metadata(link).is_err());
    assert_eq!(
        fs::read(tree.path("outside/secret")).expect("read outside target"),
        b"secret"
    );
}

#[test]
fn scenario_rm_09() {
    let tree = TempTree::new("rm09");
    tree.dir("sub");
    let link = tree.path("ld");
    symlink_dir("sub", &link).expect("create directory link");
    let result = remove_symlink_file(&link);

    #[cfg(unix)]
    {
        result.expect("Unix link has no kind");
        assert!(fs::symlink_metadata(link).is_err());
    }
    #[cfg(windows)]
    {
        assert_refused(result, &link);
        assert_link(&link);
    }
}

#[test]
fn scenario_rm_10() {
    let tree = TempTree::new("rm10");
    tree.file("a", b"a");
    let link = tree.path("la");
    symlink_file("a", &link).expect("create file link");
    let result = remove_symlink_dir(&link);

    #[cfg(unix)]
    {
        result.expect("Unix link has no kind");
        assert!(fs::symlink_metadata(link).is_err());
    }
    #[cfg(windows)]
    {
        assert_refused(result, &link);
        assert_link(&link);
    }
}

#[cfg(windows)]
#[test]
fn scenario_rm_11() {
    let tree = TempTree::new("rm11");
    let target = tree.file("sub/x", b"target");
    let target_dir = target.parent().expect("target parent");
    let junction = tree.path("jn");
    create_junction(target_dir, &junction);

    remove_symlink_auto(&junction).expect("remove junction automatically");

    assert!(fs::symlink_metadata(junction).is_err());
    assert_eq!(fs::read(target).expect("read junction target"), b"target");
}

#[cfg(windows)]
#[test]
fn scenario_rm_12() {
    let tree = TempTree::new("rm12");
    let target = tree.dir("sub");
    let junction = tree.path("jn");
    create_junction(&target, &junction);

    remove_symlink_dir(&junction).expect("remove junction as directory link");

    assert!(fs::symlink_metadata(junction).is_err());
    assert!(target.is_dir());
}

#[cfg(windows)]
#[test]
fn scenario_rm_13() {
    let tree = TempTree::new("rm13");
    let target = tree.dir("sub");
    let junction = tree.path("jn");
    create_junction(&target, &junction);

    assert_refused(remove_symlink_file(&junction), &junction);

    assert_link(&junction);
    assert!(target.is_dir());
}

#[test]
fn scenario_rm_14() {
    let tree = TempTree::new("rm14");
    let path = tree.path("nope");

    for error in [
        remove_symlink_file(&path).expect_err("missing file link"),
        remove_symlink_dir(&path).expect_err("missing directory link"),
        remove_symlink_auto(&path).expect_err("missing automatic link"),
    ] {
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }
}

#[test]
fn scenario_rm_15() {
    let error = remove_symlink_auto("la\0x").expect_err("interior nul must fail");
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
}

#[test]
fn scenario_rf_01() {
    let tree = TempTree::new("rf01");
    let path = tree.file("real", b"preserve");

    assert_refused(remove_symlink_auto(&path), &path);

    assert_eq!(fs::read(path).expect("read preserved file"), b"preserve");
}

#[test]
fn scenario_rf_02() {
    let tree = TempTree::new("rf02");
    let path = tree.file("real", b"preserve");

    assert_refused(remove_symlink_file(&path), &path);

    assert_eq!(fs::read(path).expect("read preserved file"), b"preserve");
}

#[test]
fn scenario_rf_03() {
    let tree = TempTree::new("rf03");
    let path = tree.file("real", b"preserve");

    assert_refused(remove_symlink_dir(&path), &path);

    assert_eq!(fs::read(path).expect("read preserved file"), b"preserve");
}

#[test]
fn scenario_rf_04() {
    let tree = TempTree::new("rf04");
    let path = tree.dir("empty");

    assert_refused(remove_symlink_dir(&path), &path);

    assert!(path.is_dir());
}

#[test]
fn scenario_rf_05() {
    let tree = TempTree::new("rf05");
    let path = tree.dir("empty");

    assert_refused(remove_symlink_auto(&path), &path);

    assert!(path.is_dir());
}

#[test]
fn scenario_rf_06() {
    let tree = TempTree::new("rf06");
    let child = tree.file("full/x", b"preserve");
    let path = tree.path("full");

    assert_refused(remove_symlink_file(&path), &path);

    assert_eq!(fs::read(child).expect("read preserved child"), b"preserve");
}

#[test]
fn scenario_rf_07() {
    let tree = TempTree::new("rf07");
    let child = tree.file("full/x", b"preserve");
    let path = tree.path("full");

    assert_refused(remove_symlink_dir(&path), &path);

    assert_eq!(fs::read(child).expect("read preserved child"), b"preserve");
}

// manual_scenario_rf_08: On Windows, pass a non-surrogate reparse point to
// remove_symlink_auto. It must return InvalidInput and preserve the path.

#[test]
fn scenario_rf_09() {
    let tree = TempTree::new("rf09");
    let path = tree.file("latest.log", b"current");
    tree.file("new.log", b"new");

    assert_refused(remove_symlink_file(&path), &path);
    let create_error = symlink_file("new.log", &path).expect_err("occupied path must remain");

    assert_eq!(create_error.kind(), io::ErrorKind::AlreadyExists);
    assert_eq!(fs::read(path).expect("read preserved log"), b"current");
}

#[test]
fn scenario_np_01() {
    let tree = TempTree::new("np01");
    let link = tree.path("link");

    assert_no_panic(|| {
        let _ = symlink_file("", &link);
    });
    assert_no_panic(|| {
        let _ = symlink_file("target", "");
    });
    assert_no_panic(|| {
        let _ = symlink_dir("", &link);
    });
    assert_no_panic(|| {
        let _ = symlink_dir("target", "");
    });
    assert_no_panic(|| {
        let _ = symlink_auto("", &link);
    });
    assert_no_panic(|| {
        let _ = symlink_auto("target", "");
    });
    assert_no_panic(|| {
        let _ = remove_symlink_file("");
    });
    assert_no_panic(|| {
        let _ = remove_symlink_dir("");
    });
    assert_no_panic(|| {
        let _ = remove_symlink_auto("");
    });
}

#[test]
fn scenario_np_02() {
    let tree = TempTree::new("np02");
    let link = tree.path("link");

    for target in ["a\0b", "\0"] {
        assert_no_panic(|| {
            let _ = symlink_file(target, &link);
        });
        assert_no_panic(|| {
            let _ = symlink_dir(target, &link);
        });
        assert_no_panic(|| {
            let _ = symlink_auto(target, &link);
        });
    }
    assert_no_panic(|| {
        let _ = symlink_file("target", "a\0b");
    });
    assert_no_panic(|| {
        let _ = symlink_dir("target", "a\0b");
    });
    assert_no_panic(|| {
        let _ = symlink_auto("target", "a\0b");
    });
    assert_no_panic(|| {
        let _ = remove_symlink_file("a\0b");
    });
    assert_no_panic(|| {
        let _ = remove_symlink_dir("a\0b");
    });
    assert_no_panic(|| {
        let _ = remove_symlink_auto("a\0b");
    });
}

fn exercise_oversized_path(path: &Path, tree: &TempTree, label: &str) {
    let file_link = tree.path(&format!("{label}-file"));
    let dir_link = tree.path(&format!("{label}-dir"));
    let auto_link = tree.path(&format!("{label}-auto"));

    assert_no_panic(|| {
        let _ = symlink_file(path, &file_link);
    });
    assert_no_panic(|| {
        let _ = symlink_dir(path, &dir_link);
    });
    assert_no_panic(|| {
        let _ = symlink_auto(path, &auto_link);
    });
    assert_no_panic(|| {
        let _ = symlink_file("target", path);
    });
    assert_no_panic(|| {
        let _ = symlink_dir("target", path);
    });
    assert_no_panic(|| {
        let _ = symlink_auto("target", path);
    });
    assert_no_panic(|| {
        let _ = remove_symlink_file(path);
    });
    assert_no_panic(|| {
        let _ = remove_symlink_dir(path);
    });
    assert_no_panic(|| {
        let _ = remove_symlink_auto(path);
    });
}

#[test]
fn scenario_np_03() {
    let tree = TempTree::new("np03");
    let large_component = tree.path(&"x".repeat(4096));
    let deep_path = tree.root.join(vec!["p"; 300].join("/"));

    exercise_oversized_path(&large_component, &tree, "large");
    exercise_oversized_path(&deep_path, &tree, "deep");
}

#[test]
fn scenario_np_04() {
    #[cfg(unix)]
    let root = Path::new("/");
    #[cfg(windows)]
    let root = Path::new(r"C:\");

    assert_no_panic(|| {
        let _ = symlink_file(root, root);
    });
    assert_no_panic(|| {
        let _ = symlink_dir(root, root);
    });
    assert_no_panic(|| {
        let _ = symlink_auto(root, root);
    });
    assert_no_panic(|| {
        let _ = remove_symlink_file(root);
    });
    assert_no_panic(|| {
        let _ = remove_symlink_dir(root);
    });
    assert_no_panic(|| {
        let _ = remove_symlink_auto(root);
    });
}

#[test]
fn scenario_np_05() {
    let tree = TempTree::new("np05");
    tree.file("parent", b"file");
    let path = tree.path("parent/child");

    assert_no_panic(|| {
        let _ = remove_symlink_file(&path);
    });
    assert_no_panic(|| {
        let _ = remove_symlink_dir(&path);
    });
    assert_no_panic(|| {
        let _ = remove_symlink_auto(&path);
    });
}

#[test]
fn scenario_np_06() {
    let tree = TempTree::new("np06");
    let path = tree.path("same");

    assert_no_panic(|| {
        let _ = symlink_auto(&path, &path);
    });
}

#[test]
fn scenario_np_07() {
    let tree = TempTree::new("np07");
    let loop_link = tree.path("loop");
    symlink_file("loop", &loop_link).expect("create self-referential link");
    let link = tree.path("second");

    assert_no_panic(|| {
        let _ = symlink_auto("loop", &link);
    });
}
