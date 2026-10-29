//! Copy-on-save and input guard cases every page op must pass. A test binary
//! lists its ops in a table and calls [`guard_tests!`](crate::guard_tests).
use super::{fixture, names, setup};
use monax_core::pages::PagesError;
use std::path::{Path, PathBuf};

/// One op as `(input, out) -> result`.
pub type Op = fn(&Path, &Path) -> Result<PathBuf, PagesError>;

pub fn invalid(r: Result<PathBuf, PagesError>) -> bool {
    matches!(r, Err(PagesError::InvalidInput(_)))
}

/// `out` naming the input itself, a symlink to it, or a `sub/../` alias.
pub fn refuses_to_write_over_input(ops: &[(&str, Op)]) {
    let (dir, three) = setup();
    let before = std::fs::read(&three).unwrap();
    let link = dir.path().join("link.pdf");
    std::os::unix::fs::symlink(&three, &link).unwrap();
    let dotted = dir.path().join("sub/../three.pdf");
    std::fs::create_dir(dir.path().join("sub")).unwrap();
    for (name, op) in ops {
        for out in [&three, &link, &dotted] {
            assert!(invalid(op(&three, out)), "{name} -> {}", out.display());
        }
    }
    assert_eq!(std::fs::read(&three).unwrap(), before);
    assert_eq!(names(dir.path()), ["link.pdf", "sub", "three.pdf"]);
}

/// A taken name gets `-1`, then `-2`; the existing file is untouched.
pub fn never_overwrites_existing_output(ops: &[(&str, Op)]) {
    let (dir, three) = setup();
    for (name, op) in ops {
        let out = dir.path().join(format!("{name}.pdf"));
        std::fs::write(&out, b"keep").unwrap();
        let got = op(&three, &out).unwrap();
        assert_eq!(got, dir.path().join(format!("{name}-1.pdf")), "{name}");
        let again = op(&three, &out).unwrap();
        assert_eq!(again, dir.path().join(format!("{name}-2.pdf")), "{name}");
        assert_eq!(std::fs::read(&out).unwrap(), b"keep", "{name}");
    }
}

/// Relative, missing-parent and directory outputs; nothing is written.
pub fn bad_out_paths_are_invalid(ops: &[(&str, Op)]) {
    let (dir, three) = setup();
    for (name, op) in ops {
        assert!(invalid(op(&three, Path::new("rel.pdf"))), "{name}");
        assert!(invalid(op(&three, &dir.path().join("no/x.pdf"))), "{name}");
        assert!(invalid(op(&three, dir.path())), "{name}");
    }
    assert_eq!(names(dir.path()), ["three.pdf"]);
}

pub fn encrypted_input_fails_without_output(ops: &[(&str, Op)]) {
    let dir = tempfile::tempdir().unwrap();
    for (name, op) in ops {
        let r = op(&fixture("encrypted.pdf"), &dir.path().join("o.pdf"));
        let ok = matches!(r, Err(PagesError::EncryptedInput));
        assert!(ok, "{name}: {r:?}");
    }
    assert!(names(dir.path()).is_empty());
}

pub fn damaged_input_fails_without_output(ops: &[(&str, Op)]) {
    let dir = tempfile::tempdir().unwrap();
    let bytes = std::fs::read(fixture("three-pages.pdf")).unwrap();
    let cut = dir.path().join("cut.pdf");
    std::fs::write(&cut, &bytes[..40]).unwrap();
    for (name, op) in ops {
        let r = op(&cut, &dir.path().join("o.pdf"));
        assert!(matches!(r, Err(PagesError::Damaged)), "{name}: {r:?}");
    }
    assert_eq!(names(dir.path()), ["cut.pdf"]);
}

/// One `#[test]` per guard case over `$ops`; each returns early (with the
/// skip notice) unless `$ready` is true.
#[macro_export]
macro_rules! guard_tests {
    ($ops:expr, $ready:expr) => {
        $crate::guard_tests!(@one $ops, $ready, refuses_to_write_over_input);
        $crate::guard_tests!(@one $ops, $ready, never_overwrites_existing_output);
        $crate::guard_tests!(@one $ops, $ready, bad_out_paths_are_invalid);
        $crate::guard_tests!(@one $ops, $ready, encrypted_input_fails_without_output);
        $crate::guard_tests!(@one $ops, $ready, damaged_input_fails_without_output);
    };
    (@one $ops:expr, $ready:expr, $case:ident) => {
        #[test]
        fn $case() {
            if $ready {
                common::guard::$case(&$ops);
            }
        }
    };
}
