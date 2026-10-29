//! Passwords never reach qpdf's argv (R86): a recording qpdf double sees
//! them only inside 0600 temp files, which are gone after success and after
//! failure, as are the decrypted temp copies; no error or note carries them.
mod common;
use common::{fixture, setup};
use monax_core::pages::{PagesError, RotateInput};
use monax_core::security::{self, EncryptOpts, Permissions};
use serde_json::json;
use std::path::Path;

const USER: &str = "u-Sesame-α 1";
const OWNER: &str = "o-Sesame-β 2";
const WRONG: &str = "w-Sesame-γ 3";

fn rotate(input: &Path, password: &str) -> Result<(std::path::PathBuf, Vec<String>), PagesError> {
    let v = json!({ "input": input, "pages": [0], "degrees": 90, "password": password });
    serde_json::from_value::<RotateInput>(v).unwrap().run()
}

// Every step mutates process env, so this binary has one test.
#[test]
fn passwords_reach_qpdf_only_through_private_temp_files() {
    require_qpdf!();
    let real = monax_sidecar::find(monax_sidecar::Tool::Qpdf).unwrap();
    let (dir, three) = setup();
    let record = dir.path().join("record.txt");
    // SAFETY: the only test in this binary.
    unsafe {
        std::env::set_var("MONAX_REAL_QPDF", &real);
        std::env::set_var("MONAX_QPDF_RECORD", &record);
        std::env::set_var("MONAX_BIN_DIR", fixture("bin-record"));
    }
    let opts = EncryptOpts {
        user_pw: USER.into(),
        owner_pw: OWNER.into(),
        permissions: Permissions::default(),
        aes256: true,
    };
    let enc = security::encrypt(&three, &dir.path().join("enc.pdf"), opts).unwrap();
    security::decrypt(&enc, &dir.path().join("dec.pdf"), USER).unwrap();
    let info = security::inspect(&enc, Some(OWNER)).unwrap();
    assert_eq!(info.method.as_deref(), Some("AES-256"));
    let (_, notes) = rotate(&enc, USER).unwrap();
    let mut errors = vec![format!("{info:?}"), notes.join("\n")];
    for r in [
        security::decrypt(&enc, &dir.path().join("x.pdf"), WRONG).map(|_| ()),
        security::inspect(&enc, Some(WRONG)).map(|_| ()),
        rotate(&enc, WRONG).map(|_| ()),
    ] {
        assert!(matches!(r, Err(PagesError::WrongPassword)), "{r:?}");
        errors.push(format!("{r:?} {}", r.as_ref().unwrap_err()));
    }
    for e in &errors {
        assert!(!e.contains("Sesame"), "an error or note carries a secret");
    }

    let log = std::fs::read_to_string(&record).unwrap();
    let lines: Vec<&str> = log.lines().collect();
    let args: Vec<&str> = lines
        .iter()
        .filter_map(|l| l.strip_prefix("arg "))
        .collect();
    let leaked = args.iter().filter(|a| a.contains("Sesame")).count();
    assert_eq!(leaked, 0, "arguments carrying a secret");
    let files: Vec<(&str, &str)> = lines
        .iter()
        .filter_map(|l| l.strip_prefix("file ")?.split_once(' '))
        .collect();
    assert!(files.len() >= 8, "{} secret files", files.len());
    for (mode, path) in &files {
        assert_eq!(*mode, "600", "{path}");
        assert!(!Path::new(path).exists(), "left behind: {path}");
    }
    let secrets: Vec<&str> = lines
        .iter()
        .filter_map(|l| l.strip_prefix("line "))
        .collect();
    for want in [USER, WRONG, OWNER] {
        assert!(secrets.contains(&want), "a password never reached qpdf");
    }
    assert!(secrets.contains(&format!("--user-password={USER}").as_str()));
    assert!(secrets.contains(&format!("--owner-password={OWNER}").as_str()));

    // Decrypted copies an op reads: private (0600), removed afterwards.
    let mut copies: Vec<(&str, &str)> = lines
        .iter()
        .filter_map(|l| l.strip_prefix("last ")?.split_once(' '))
        .filter(|(_, p)| p.ends_with("/enc.pdf") && !p.starts_with(&*dir.path().to_string_lossy()))
        .collect();
    copies.dedup(); // the op's own preflight sees the copy again
    assert_eq!(copies.len(), 1, "{copies:?}");
    assert_eq!(copies[0].0, "600");
    assert!(
        !Path::new(copies[0].1).exists(),
        "decrypted copy left behind"
    );

    // A rejected output is refused before anything is decrypted.
    std::fs::remove_file(&record).unwrap();
    let v = json!({ "input": enc, "pages": [0], "degrees": 90, "password": USER, "out": enc });
    let e = serde_json::from_value::<RotateInput>(v)
        .unwrap()
        .run()
        .unwrap_err();
    assert!(matches!(e, PagesError::InvalidInput(_)), "{e:?}");
    let log = std::fs::read_to_string(&record).unwrap_or_default();
    assert!(
        !log.contains("arg --decrypt"),
        "decrypted before the out check"
    );
}
