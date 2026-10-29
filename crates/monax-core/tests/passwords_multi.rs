//! Passwords for multi-input ops and owner-only files (R88): one password
//! per input, dropped permissions noted, guards on the original input.
mod common;
use common::docs::requires_password;
use common::{copy_fixture, fixture, names, words};
use monax_core::pages::PagesError;
use serde_json::json;
use std::path::PathBuf;

const NOTE: &str = "output is not password-protected";

/// `encrypted.pdf` (user password "user") copied as `enc.pdf`.
fn enc() -> (tempfile::TempDir, PathBuf) {
    copy_fixture("encrypted.pdf", "enc.pdf")
}

#[test]
fn merge_and_alternate_mix_take_one_password_per_input() {
    require_qpdf!();
    use monax_core::pages::{AlternateMixInput, MergeInput};
    let (dir, p) = enc();
    let (_d2, aes) = copy_fixture("aes128.pdf", "aes.pdf");
    let (out, notes) = run_input!(
        MergeInput,
        json!({ "inputs": [p, aes], "passwords": ["user", "owner"] })
    )
    .unwrap();
    assert_eq!(
        (out.clone(), notes),
        (dir.path().join("enc-merge.pdf"), vec![NOTE.to_string()])
    );
    assert_eq!(words(&out).len(), 6);
    let v = json!({ "a": p, "b": aes, "passwords": ["user", "user"] });
    let (out, _) = run_input!(AlternateMixInput, v).unwrap();
    assert_eq!(words(&out), ["one", "one", "two", "two", "three", "three"]);
}

#[test]
fn owner_only_inputs_open_without_a_password_and_note_dropped_permissions() {
    require_qpdf!();
    use monax_core::content::LinearizeInput;
    use monax_core::pages::{AlternateMixInput, MergeInput, NupInput, RotateInput};
    use monax_core::read::ReadTextInput;
    let (dir, own) = copy_fixture("owner-only.pdf", "own.pdf");
    let three = dir.path().join("three.pdf");
    std::fs::copy(fixture("three-pages.pdf"), &three).unwrap();
    let dropped = format!("permissions of {} were not carried over", own.display());
    let (out, notes) = run_input!(
        RotateInput,
        json!({ "input": own, "pages": [0], "degrees": 90 })
    )
    .unwrap();
    assert_eq!(
        (requires_password(&out), notes),
        (Some(2), vec![dropped.clone()])
    );
    let (out, notes) = run_input!(LinearizeInput, json!({ "input": own })).unwrap();
    assert_eq!(
        (requires_password(&out), notes),
        (Some(2), vec![dropped.clone()])
    );
    let (_, notes) = run_input!(NupInput, json!({ "input": own, "per_sheet": 2 })).unwrap();
    assert_eq!(notes, std::slice::from_ref(&dropped));
    let (_, notes) = run_input!(ReadTextInput, json!({ "input": own })).unwrap();
    assert_eq!(notes, std::slice::from_ref(&dropped));
    let (_, notes) = run_input!(MergeInput, json!({ "inputs": [three, own] })).unwrap();
    assert_eq!(notes, std::slice::from_ref(&dropped));
    let (_, notes) = run_input!(AlternateMixInput, json!({ "a": own, "b": three })).unwrap();
    assert_eq!(notes, [dropped]);
    let (_, notes) = run_input!(
        MergeInput,
        json!({ "inputs": [three, three], "passwords": ["unused"] })
    )
    .unwrap();
    assert!(
        notes.is_empty(),
        "a password for a plain file is ignored: {notes:?}"
    );
}

#[test]
fn guards_apply_to_the_original_input() {
    require_qpdf!();
    use monax_core::pages::{ExtractInput, SplitHalfInput};
    let (dir, p) = enc();
    let before = std::fs::read(&p).unwrap();
    let link = dir.path().join("link.pdf");
    std::os::unix::fs::symlink(&p, &link).unwrap();
    for out in [&p, &link] {
        let v = json!({ "input": p, "pages": [0], "password": "user", "out": out });
        let e = run_input!(ExtractInput, v).unwrap_err().to_string();
        assert_eq!(
            e,
            format!(
                "invalid input: output would overwrite an input: {}",
                out.display()
            )
        );
    }
    let r = run_input!(
        ExtractInput,
        json!({ "input": p, "pages": [7], "password": "user" })
    );
    assert!(
        matches!(r, Err(PagesError::PageOutOfRange { page: 7, count: 3 })),
        "{r:?}"
    );
    let missing = dir.path().join("none.pdf");
    let e = run_input!(
        SplitHalfInput,
        json!({ "input": missing, "password": "user" })
    )
    .unwrap_err();
    assert_eq!(
        e.to_string(),
        format!("invalid input: file does not exist: {}", missing.display())
    );
    assert_eq!(std::fs::read(&p).unwrap(), before);
    assert_eq!(names(dir.path()), ["enc.pdf", "link.pdf"]);
}

#[test]
fn split_names_and_out_dirs_are_checked_against_the_original() {
    require_qpdf!();
    use monax_core::pages::{SplitHalfInput, SplitPagesInput};
    use monax_core::pdfium::ExtractImagesInput;
    let (dir, p) = enc();
    let ranges = json!([{ "start": 0, "end": 2 }]);
    for pw in [json!(null), json!("user")] {
        let v =
            json!({ "input": p, "ranges": ranges, "pattern": "%[filename].pdf", "password": pw });
        let e = run_input!(SplitPagesInput, v).unwrap_err().to_string();
        let want = format!(
            "invalid input: output would overwrite an input: {}",
            p.display()
        );
        let want = if pw.is_null() {
            "encrypted input: a password is required".into()
        } else {
            want
        };
        assert_eq!(e, want);
    }
    let rel = json!({ "input": p, "ranges": ranges, "out_dir": "rel", "password": "user" });
    let e = run_input!(SplitPagesInput, rel).unwrap_err().to_string();
    assert!(
        e.starts_with("invalid input: output directory must be"),
        "{e}"
    );
    let e = run_input!(
        ExtractImagesInput,
        json!({ "input": p, "out_dir": "rel", "password": "user" })
    );
    assert!(
        e.unwrap_err()
            .to_string()
            .starts_with("invalid input: output directory must be")
    );
    let e = run_input!(
        SplitHalfInput,
        json!({ "input": p, "out": "rel.pdf", "password": "user" })
    );
    assert_eq!(
        e.unwrap_err().to_string(),
        "invalid input: output path must be absolute: rel.pdf"
    );
    assert_eq!(names(dir.path()), ["enc.pdf"]);
}

#[test]
fn encrypt_re_protects_a_password_protected_input() {
    require_qpdf!();
    use monax_core::security::EncryptInput;
    let (dir, p) = enc();
    let opts = json!({ "user_pw": "new-u", "owner_pw": "new-o" });
    let v = json!({ "input": p, "opts": opts, "password": "user" });
    let (out, notes) = run_input!(EncryptInput, v).unwrap();
    assert_eq!(
        (out.clone(), notes),
        (dir.path().join("enc-encrypt.pdf"), vec![])
    );
    assert_eq!(requires_password(&out), Some(0));
    let e = monax_core::security::inspect(&out, Some("user")).unwrap_err();
    assert_eq!(e.to_string(), "wrong password");
    assert!(monax_core::security::inspect(&out, Some("new-u")).is_ok());
}
