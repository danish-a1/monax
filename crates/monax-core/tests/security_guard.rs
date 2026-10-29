//! Copy-on-save and input guards of the security ops. encrypt and redact
//! take the shared harness; decrypt and unlock need encrypted input, so their cases
//! run on the encrypted fixtures here.
mod common;
use common::guard::{Op, invalid};
use common::{copy_fixture, names};
use monax_core::pages::Rect;
use monax_core::security::{self, EncryptOpts, Permissions, Redaction};
use std::path::PathBuf;

const OPS: [(&str, Op); 2] = [
    ("encrypt", |i, o| {
        let opts = EncryptOpts {
            user_pw: "u".into(),
            owner_pw: "o".into(),
            permissions: Permissions::default(),
            aes256: true,
        };
        security::encrypt(i, o, opts)
    }),
    ("redact", |i, o| {
        let rect = Rect {
            left: 0.0,
            bottom: 0.0,
            right: 50.0,
            top: 50.0,
        };
        security::redact(i, o, &[Redaction { page: 1, rect }])
    }),
];

guard_tests!(OPS, common::have_pdfium() && common::have_qpdf());

#[test]
fn decrypt_and_unlock_guard_their_output() {
    require_qpdf!();
    let (dir, enc) = copy_fixture("encrypted.pdf", "enc.pdf");
    let own = dir.path().join("own.pdf");
    std::fs::copy(common::fixture("owner-only.pdf"), &own).unwrap();
    let link = dir.path().join("link.pdf");
    std::os::unix::fs::symlink(&own, &link).unwrap();
    let (rel, orphan, d) = (
        PathBuf::from("rel.pdf"),
        dir.path().join("no/x.pdf"),
        dir.path(),
    );
    let before = (std::fs::read(&enc).unwrap(), std::fs::read(&own).unwrap());
    for out in [&enc, &rel, &orphan, &d.to_path_buf()] {
        assert!(
            invalid(security::decrypt(&enc, out, "user")),
            "{}",
            out.display()
        );
    }
    for out in [&own, &link, &rel, &orphan, &d.to_path_buf()] {
        assert!(invalid(security::unlock(&own, out)), "{}", out.display());
    }
    let after = (std::fs::read(&enc).unwrap(), std::fs::read(&own).unwrap());
    assert_eq!(after, before);
    assert_eq!(names(d), ["enc.pdf", "link.pdf", "own.pdf"]);
}

#[test]
fn inspect_guards_its_report_and_reports_damage() {
    require_qpdf!();
    use monax_core::pages::PagesError;
    use monax_core::security::InspectSecurityInput;
    let (dir, own) = copy_fixture("owner-only.pdf", "own.pdf");
    let d = dir.path();
    let link = d.join("link.pdf");
    std::os::unix::fs::symlink(&own, &link).unwrap();
    let run = |input: &PathBuf, out: &PathBuf| {
        let v = serde_json::json!({ "input": input, "out": out });
        serde_json::from_value::<InspectSecurityInput>(v)
            .unwrap()
            .run()
    };
    let before = std::fs::read(&own).unwrap();
    for out in [
        own.clone(),
        link.clone(),
        "rel.json".into(),
        d.join("no/x.json"),
        d.to_path_buf(),
    ] {
        assert!(invalid(run(&own, &out)), "{}", out.display());
    }
    let taken = d.join("r.json");
    std::fs::write(&taken, "keep").unwrap();
    assert_eq!(run(&own, &taken).unwrap(), d.join("r-1.json"));
    assert_eq!(std::fs::read_to_string(&taken).unwrap(), "keep");
    let cut = d.join("cut.pdf");
    std::fs::write(&cut, b"%PDF-1.4\n").unwrap();
    let r = run(&cut, &d.join("c.json"));
    assert!(matches!(r, Err(PagesError::Damaged)), "{r:?}");
    assert_eq!(std::fs::read(&own).unwrap(), before);
    assert_eq!(
        names(d),
        ["cut.pdf", "link.pdf", "own.pdf", "r-1.json", "r.json"]
    );
}
