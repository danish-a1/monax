//! compress, set_metadata, repair, split_size, merge and the form check
//! before a render (R118: qpdf's rewrite of a form) keep their temp copies
//! of document data private (0600 files in a 0700 dir), even under a
//! shared (0755) temp root, and remove them afterwards, refused or not.
mod common;
use common::probe14::probe14_pdf;
use common::scan::scan_pdf;
use common::{fixture, setup};
use monax_core::Secret;
use monax_core::content::{self, CompressOpts, Metadata};
use monax_core::pages::{self, MergeOpts};
use monax_core::pdfium::{render, thumbnail_png_with_password};
use monax_core::security::{EncryptOpts, Permissions, encrypt};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

/// The job's own files qpdf was handed since the last call (under `tmp`,
/// the temp root): `(file mode, dir mode, path)`; empties the record.
fn ours(record: &Path, tmp: &Path) -> Vec<(String, String, String)> {
    let log = std::fs::read_to_string(record).unwrap_or_default();
    std::fs::write(record, "").unwrap();
    log.lines()
        .filter_map(|l| {
            let (mode, rest) = l.strip_prefix("mode ")?.split_once(' ')?;
            let (dmode, path) = rest.split_once(' ')?;
            Some((mode.into(), dmode.into(), path.into()))
        })
        .filter(|(_, _, p): &(String, String, String)| p.starts_with(&*tmp.to_string_lossy()))
        .collect()
}

// Every step mutates process env, so this binary has one test.
#[test]
fn content_ops_keep_their_temp_files_private_and_remove_them() {
    require_qpdf!();
    let real = monax_sidecar::find(monax_sidecar::Tool::Qpdf).unwrap();
    let (dir, three) = setup();
    let dag = probe14_pdf(dir.path(), "fielddag:3");
    let record = dir.path().join("record.txt");
    let tmp = dir.path().join("tmp");
    std::fs::create_dir(&tmp).unwrap();
    std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755)).unwrap();
    // SAFETY: the only test in this binary.
    unsafe {
        std::env::set_var("MONAX_REAL_QPDF", &real);
        std::env::set_var("MONAX_QPDF_RECORD", &record);
        std::env::set_var("MONAX_BIN_DIR", fixture("bin-record"));
        std::env::set_var("TMPDIR", &tmp);
    }
    let d = dir.path();
    let scan = scan_pdf(d, 600, 144);
    let opts = CompressOpts {
        dpi: 72,
        discard_media: true,
        ..CompressOpts::default()
    };
    let title = Metadata {
        title: Some("T".into()),
        ..Metadata::default()
    };
    let mut bad = Vec::new();
    let mut check = |op: &str, ok: bool| {
        assert!(ok, "{op} failed");
        let files = ours(&record, &tmp);
        if files.is_empty() {
            bad.push(format!("{op}: no private temp file handed to qpdf"));
        }
        for (mode, dmode, path) in files {
            if (mode.as_str(), dmode.as_str()) != ("600", "700") {
                bad.push(format!("{op}: {mode} in {dmode}: {path}"));
            }
            if Path::new(&path).exists() {
                bad.push(format!("{op}: left behind: {path}"));
            }
        }
    };
    check(
        "compress",
        content::compress(&scan, &d.join("c.pdf"), opts).is_ok(),
    );
    let m = content::set_metadata(&three, &d.join("m.pdf"), title);
    check("set_metadata", m.is_ok());
    check("repair", content::repair(&three, &d.join("r.pdf")).is_ok());
    let s = pages::split_size(&three, 1, d, "s-%[index].pdf");
    check("split_size", s.is_ok());
    let both = [three.clone(), three.clone()];
    let opts = MergeOpts {
        blank_before_even: true,
    };
    check("merge", pages::merge(&both, &d.join("j.pdf"), opts).is_ok());
    let refused = content::flatten(&dag, &d.join("f.pdf")).is_err();
    check("flatten refused", refused);
    assert!(bad.is_empty(), "{bad:#?}");
    form_copy_is_cached_privately_until_replaced(d, &three, &record, &tmp);
}

/// R118, R122: the last form drawn keeps its checked copy only as an open
/// handle (nothing on disk) while the file is unchanged; a changed file is
/// rewritten again, and a call with a password neither uses nor stores it.
fn form_copy_is_cached_privately_until_replaced(d: &Path, plain: &Path, rec: &Path, tmp: &Path) {
    let form = d.join("form.pdf");
    std::fs::copy(fixture("annots.pdf"), &form).unwrap();
    let draw = |p: &Path| render(p, 0, 72).unwrap();
    let forms =
        |v: &[(String, String, String)]| v.iter().filter(|f| f.2.ends_with("/form.pdf")).count();
    draw(&form);
    let first = ours(rec, tmp);
    let copy = first.iter().find(|f| f.2.ends_with("/form.pdf")).unwrap();
    assert_eq!((&*copy.0, &*copy.1), ("600", "700"), "{copy:?}");
    let on_disk: Vec<_> = first.iter().filter(|f| Path::new(&f.2).exists()).collect();
    assert!(on_disk.is_empty(), "cached copy on disk: {on_disk:?}");
    draw(&form); // drawn from the cached handle
    assert_eq!(ours(rec, tmp), [], "unchanged form rewritten again");
    let later = std::time::SystemTime::now() + std::time::Duration::from_secs(5);
    let f = std::fs::File::options().write(true).open(&form).unwrap();
    f.set_modified(later).unwrap();
    draw(&form);
    assert!(forms(&ours(rec, tmp)) > 0, "changed form not rewritten");
    // An owner-only form opened with its owner password, the cache holding it.
    let owner = EncryptOpts {
        user_pw: "".into(),
        owner_pw: "own".into(),
        permissions: Permissions {
            modify: false,
            ..Permissions::default()
        },
        aes256: true,
    };
    let own = encrypt(&form, &d.join("own.pdf"), owner).unwrap();
    // Served: the cache holds `own`, the password call must rewrite it.
    draw(&own);
    ours(rec, tmp);
    let pw = Secret::new("own");
    let thumb = || thumbnail_png_with_password(&own, 1, 64, Some(&pw)).unwrap();
    thumb();
    assert!(forms(&ours(rec, tmp)) > 0, "password call served the cache");
    // Stored: the cache holds `form`; a password call must not replace it.
    draw(&form);
    ours(rec, tmp);
    thumb();
    ours(rec, tmp);
    draw(&form);
    assert_eq!(
        forms(&ours(rec, tmp)),
        0,
        "password call replaced the cache"
    );
    draw(plain);
}
