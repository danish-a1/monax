//! inspect, and the security input structs from client JSON: defaults,
//! notes, the inspect report, and secrets marked write-only in the schema.
mod common;
use common::{copy_fixture, fixture, names};
use monax_core::pages::PagesError;
use monax_core::security::{
    self, DecryptInput, EncryptInput, InspectSecurityInput, Permissions, Print, SecurityInfo,
    UnlockInput,
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

fn input<T: DeserializeOwned>(v: Value) -> T {
    serde_json::from_value(v).unwrap()
}

#[test]
fn inputs_run_into_their_default_outputs_with_notes() {
    require_qpdf!();
    let (dir, p) = copy_fixture("three-pages.pdf", "three.pdf");
    let d = dir.path();
    let opts =
        json!({ "user_pw": "u", "owner_pw": "o", "permissions": { "accessibility": false } });
    let (out, notes) = input::<EncryptInput>(json!({ "input": p, "opts": opts }))
        .run()
        .unwrap();
    assert_eq!(out, d.join("three-encrypt.pdf"));
    assert_eq!(
        notes,
        ["accessibility cannot be restricted: AES encryption always allows it"]
    );
    let v = json!({ "input": out, "password": "u" });
    let (dec, notes) = input::<DecryptInput>(v).run().unwrap();
    assert_eq!((dec, notes), (d.join("three-encrypt-decrypt.pdf"), vec![]));
    let (_d, own) = copy_fixture("owner-only.pdf", "own.pdf");
    let (out, _) = input::<UnlockInput>(json!({ "input": own })).run().unwrap();
    assert_eq!(out, own.with_file_name("own-unlock.pdf"));
    let e = input::<EncryptInput>(json!({ "input": p, "opts": { "owner_pw": "o" } })).run();
    let want = "invalid input: nothing to protect: set a user password or restrict a permission";
    assert_eq!(e.unwrap_err().to_string(), want);
    let names = names(d);
    assert_eq!(
        names,
        [
            "three-encrypt-decrypt.pdf",
            "three-encrypt.pdf",
            "three.pdf"
        ]
    );
}

#[test]
fn inspect_writes_its_report_as_json() {
    require_qpdf!();
    let (dir, own) = copy_fixture("owner-only.pdf", "own.pdf");
    let out = input::<InspectSecurityInput>(json!({ "input": own }))
        .run()
        .unwrap();
    assert_eq!(out, dir.path().join("own-security.json"));
    let v: Value = serde_json::from_slice(&std::fs::read(&out).unwrap()).unwrap();
    let perms = json!({ "print": "low", "modify": true, "extract": false, "annotate": true,
        "fill_forms": true, "assemble": true, "accessibility": true });
    let want = json!({ "encrypted": true, "needs_password": false, "method": "AES-256", "permissions": perms });
    assert_eq!(v, want);
    let (_d, enc) = copy_fixture("encrypted.pdf", "enc.pdf");
    let r = input::<InspectSecurityInput>(json!({ "input": enc, "password": "nope" })).run();
    assert_eq!(r.unwrap_err().to_string(), "wrong password");
}

#[test]
fn secrets_are_write_only_in_the_schemas() {
    let schema = |s: schemars::Schema| s.as_value().clone();
    let enc = schema(schemars::schema_for!(EncryptInput));
    let opts = &enc["$defs"]["EncryptOpts"]["properties"];
    assert_eq!(
        (
            &opts["user_pw"]["writeOnly"],
            &opts["owner_pw"]["writeOnly"]
        ),
        (&json!(true), &json!(true))
    );
    for s in [
        schema(schemars::schema_for!(DecryptInput)),
        schema(schemars::schema_for!(InspectSecurityInput)),
        schema(schemars::schema_for!(monax_core::pages::RotateInput)),
        schema(schemars::schema_for!(monax_core::read::ReadTextInput)),
    ] {
        assert_eq!(s["properties"]["password"]["writeOnly"], true, "{s}");
    }
    let merge = schema(schemars::schema_for!(monax_core::pages::MergeInput));
    assert_eq!(merge["properties"]["passwords"]["writeOnly"], true);
    assert_eq!(
        schema(schemars::schema_for!(DecryptInput))["required"],
        json!(["input", "password"])
    );
}

fn info(name: &str, pw: Option<&str>) -> Result<SecurityInfo, PagesError> {
    security::inspect(&fixture(name), pw)
}

#[test]
fn inspect_reports_method_and_permissions() {
    require_qpdf!();
    let plain = info("three-pages.pdf", None).unwrap();
    let none = SecurityInfo {
        encrypted: false,
        needs_password: false,
        method: None,
        permissions: None,
    };
    assert_eq!(plain, none);
    let locked = info("encrypted.pdf", None).unwrap();
    assert_eq!(
        locked,
        SecurityInfo {
            encrypted: true,
            needs_password: true,
            ..none.clone()
        }
    );
    let open = info("encrypted.pdf", Some("user")).unwrap();
    assert_eq!(
        (open.needs_password, open.method.as_deref()),
        (true, Some("AES-256"))
    );
    assert_eq!(open.permissions, Some(Permissions::default()));
    assert_eq!(
        info("aes128.pdf", Some("owner")).unwrap().method.as_deref(),
        Some("AES-128")
    );
    let own = info("owner-only.pdf", None).unwrap();
    assert_eq!((own.encrypted, own.needs_password), (true, false));
    let want = Permissions {
        print: Print::Low,
        extract: false,
        ..Permissions::default()
    };
    assert_eq!(own.permissions, Some(want));
    let r = info("encrypted.pdf", Some("o-Sesame wrong"));
    assert!(matches!(r, Err(PagesError::WrongPassword)), "{r:?}");
    let r = info("owner-only.pdf", Some("o-Sesame wrong"));
    assert!(matches!(r, Err(PagesError::WrongPassword)), "{r:?}");
}

#[test]
fn secrets_never_show_in_debug_or_deserialize_errors() {
    use monax_core::pages::{MergeInput, RotateInput};
    let pw = "x-Sesame 9";
    let rot: RotateInput =
        input(json!({ "input": "/a.pdf", "pages": [0], "degrees": 90, "password": pw }));
    let merge: MergeInput = input(json!({ "inputs": ["/a.pdf"], "passwords": [pw] }));
    let opts = json!({ "user_pw": pw, "owner_pw": pw });
    let enc: EncryptInput = input(json!({ "input": "/a.pdf", "opts": opts, "password": pw }));
    let dec: DecryptInput = input(json!({ "input": "/a.pdf", "password": pw }));
    for shown in [
        format!("{rot:?}"),
        format!("{merge:?}"),
        format!("{enc:?}"),
        format!("{dec:?}"),
    ] {
        assert!(!shown.contains("Sesame"), "secret in Debug output");
        assert!(shown.contains("***"), "{shown}");
    }
    let bad = [
        serde_json::from_value::<MergeInput>(json!({ "inputs": ["/a.pdf"], "passwords": pw }))
            .unwrap_err(),
        serde_json::from_value::<MergeInput>(json!({ "inputs": ["/a.pdf"], "passwords": [[pw]] }))
            .unwrap_err(),
        serde_json::from_value::<RotateInput>(
            json!({ "input": "/a", "pages": [], "degrees": 90, "password": [pw] }),
        )
        .unwrap_err(),
        serde_json::from_value::<DecryptInput>(
            json!({ "input": "/a.pdf", "password": { "k": pw } }),
        )
        .unwrap_err(),
    ];
    for e in bad {
        let e = e.to_string();
        assert!(!e.contains("Sesame"), "secret in a deserialize error");
        assert!(e.contains("password"), "{e}");
    }
}
