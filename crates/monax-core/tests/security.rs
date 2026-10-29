//! encrypt, decrypt and unlock against real qpdf and pdfium.
mod common;
use common::docs::{encryption, requires_password};
use common::{copy_fixture, fixture, names, setup, words};
use monax_core::pages::PagesError;
use monax_core::security::{self, EncryptOpts, Permissions, Print};
use serde_json::json;

const USER: &str = "u-Sesame 1";
const OWNER: &str = "o-Sesame 2";

fn opts(user: &str, owner: &str) -> EncryptOpts {
    EncryptOpts {
        user_pw: user.into(),
        owner_pw: owner.into(),
        permissions: Permissions::default(),
        aes256: true,
    }
}

#[test]
fn encrypt_defaults_to_aes256_with_the_requested_permissions() {
    require_qpdf!();
    let (dir, three) = setup();
    let mut o = opts(USER, OWNER);
    o.permissions = Permissions {
        print: Print::Low,
        modify: false,
        extract: false,
        annotate: true,
        fill_forms: false,
        assemble: true,
        accessibility: true,
    };
    let out = security::encrypt(&three, &dir.path().join("e.pdf"), o).unwrap();
    assert_eq!(requires_password(&out), Some(0));
    let e = encryption(&out, USER);
    assert_eq!(e["parameters"]["bits"], 256);
    assert_eq!(e["parameters"]["method"], "AESv3");
    let caps = json!({ "accessibility": true, "extract": false, "modify": false,
        "modifyannotations": true, "modifyassembly": true, "modifyforms": false,
        "modifyother": false, "printhigh": false, "printlow": true });
    assert_eq!(e["capabilities"], caps);
    assert_eq!(encryption(&out, OWNER)["ownerpasswordmatched"], true);
    assert_eq!(names(dir.path()), ["e.pdf", "three.pdf"]);
}

#[test]
fn encrypt_aes128_and_owner_only() {
    require_qpdf!();
    let (dir, three) = setup();
    let mut o = opts(USER, OWNER);
    o.aes256 = false;
    let out = security::encrypt(&three, &dir.path().join("a.pdf"), o).unwrap();
    let e = encryption(&out, USER);
    assert_eq!(
        (
            e["parameters"]["bits"].clone(),
            e["parameters"]["method"].clone()
        ),
        (json!(128), json!("AESv2"))
    );
    let mut o = opts("", OWNER);
    o.permissions.print = Print::None;
    let out = security::encrypt(&three, &dir.path().join("o.pdf"), o).unwrap();
    assert_eq!(requires_password(&out), Some(3));
    assert_eq!(encryption(&out, "")["capabilities"]["printlow"], false);
    assert_eq!(words(&out), ["one", "two", "three"]);
}

#[test]
fn encrypt_rejects_weak_settings_without_echoing_passwords() {
    require_qpdf!();
    let (dir, three) = setup();
    let out = dir.path().join("e.pdf");
    let mut a11y = opts("", OWNER);
    a11y.permissions.accessibility = false; // AES cannot restrict it
    let cases = [
        (
            a11y,
            "invalid input: nothing to protect: set a user password or restrict a permission",
        ),
        (opts(USER, ""), "invalid input: owner password is required"),
        (
            opts(OWNER, OWNER),
            "invalid input: owner password must differ from the user password",
        ),
        (
            opts("", OWNER),
            "invalid input: nothing to protect: set a user password or restrict a permission",
        ),
        (
            opts("a\nb", OWNER),
            "invalid input: password must not contain line breaks or NUL",
        ),
    ];
    for (o, want) in cases {
        let e = security::encrypt(&three, &out, o).unwrap_err().to_string();
        assert_eq!(e, want);
        assert!(!e.contains("Sesame"), "{e}");
    }
    let r = security::encrypt(&fixture("encrypted.pdf"), &out, opts(USER, OWNER));
    assert!(matches!(r, Err(PagesError::EncryptedInput)), "{r:?}");
    assert_eq!(names(dir.path()), ["three.pdf"]);
}

#[test]
fn decrypt_with_user_or_owner_password() {
    require_qpdf!();
    let (dir, aes) = copy_fixture("aes128.pdf", "aes.pdf");
    for pw in ["user", "owner"] {
        let out = security::decrypt(&aes, &dir.path().join("d.pdf"), pw).unwrap();
        assert_eq!(requires_password(&out), Some(2));
        assert_eq!(words(&out), ["one", "two", "three"]);
    }
    let enc = fixture("encrypted.pdf");
    let out = security::decrypt(&enc, &dir.path().join("d.pdf"), "user").unwrap();
    assert_eq!(requires_password(&out), Some(2));
    assert_eq!(
        names(dir.path()),
        ["aes.pdf", "d-1.pdf", "d-2.pdf", "d.pdf"]
    );
}

#[test]
fn decrypt_failures_leave_nothing() {
    require_qpdf!();
    let (dir, three) = setup();
    let out = dir.path().join("d.pdf");
    let r = security::decrypt(&fixture("encrypted.pdf"), &out, "o-Sesame wrong");
    assert!(matches!(r, Err(PagesError::WrongPassword)), "{r:?}");
    assert_eq!(r.unwrap_err().to_string(), "wrong password");
    let e = security::decrypt(&three, &out, "x")
        .unwrap_err()
        .to_string();
    assert_eq!(e, "invalid input: input is not encrypted");
    let cut = dir.path().join("cut.pdf");
    std::fs::write(&cut, b"%PDF-1.4\n").unwrap();
    let r = security::decrypt(&cut, &out, "x");
    assert!(matches!(r, Err(PagesError::Damaged)), "{r:?}");
    let r = security::decrypt(&fixture("encrypted.pdf"), &fixture("encrypted.pdf"), "user");
    assert!(matches!(r, Err(PagesError::InvalidInput(_))), "{r:?}");
    assert_eq!(names(dir.path()), ["cut.pdf", "three.pdf"]);
}

#[test]
fn unlock_lifts_owner_restrictions_only() {
    require_qpdf!();
    let (dir, own) = copy_fixture("owner-only.pdf", "own.pdf");
    let out = security::unlock(&own, &dir.path().join("u.pdf")).unwrap();
    assert_eq!(requires_password(&out), Some(2));
    assert_eq!(words(&out), ["one", "two", "three"]);
    let out = dir.path().join("x.pdf");
    let r = security::unlock(&fixture("encrypted.pdf"), &out);
    assert!(matches!(r, Err(PagesError::EncryptedInput)), "{r:?}");
    let r = security::unlock(&fixture("three-pages.pdf"), &out);
    assert_eq!(
        r.unwrap_err().to_string(),
        "invalid input: input is not encrypted"
    );
    let r = security::unlock(&own, &own);
    assert!(matches!(r, Err(PagesError::InvalidInput(_))), "{r:?}");
    assert_eq!(names(dir.path()), ["own.pdf", "u.pdf"]);
}
