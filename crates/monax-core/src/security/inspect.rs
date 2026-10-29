//! inspect: whether a PDF is encrypted, how, and what it permits.
use super::secret::{encrypted, password_arg, secret_file};
use super::{Permissions, Print};
use crate::pages::{PagesError, qpdf_out};
use crate::{check_input_file, pdfium};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::ffi::OsString;
use std::path::Path;

/// Security of a PDF as qpdf reports it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SecurityInfo {
    /// The file is encrypted (user or owner password).
    pub encrypted: bool,
    /// A user password is needed to open it.
    pub needs_password: bool,
    /// `AES-256`, `AES-128`, `RC4-128` or `RC4-40`; null when not encrypted
    /// or when it needs a password that was not given.
    pub method: Option<String>,
    /// What a reader without the owner password may do; null when not
    /// encrypted or when it needs a password that was not given.
    pub permissions: Option<Permissions>,
}

/// Reports `input`'s encryption; `password` (a secret, user or owner) opens
/// a file that needs one. Without it such a file reports only `encrypted`
/// and `needs_password`.
///
/// # Errors
/// `InvalidInput` (bad path), `WrongPassword`, `Damaged`, `Qpdf`, `Sidecar`,
/// `Pdfium`, `Io`.
pub fn inspect(input: &Path, password: Option<&str>) -> Result<SecurityInfo, PagesError> {
    check_input_file(input)?;
    let needs_password = match encrypted(input, None) {
        Ok(false) => {
            pdfium::page_count(input)?;
            return Ok(SecurityInfo {
                encrypted: false,
                needs_password: false,
                method: None,
                permissions: None,
            });
        }
        Ok(true) => false,
        Err(PagesError::EncryptedInput) => true,
        Err(e) => return Err(e),
    };
    let locked = SecurityInfo {
        encrypted: true,
        needs_password,
        method: None,
        permissions: None,
    };
    if needs_password && password.is_none() {
        return Ok(locked);
    }
    if password.is_some() {
        encrypted(input, password)?;
    }
    let file = password.map(|p| secret_file(&[p.into()])).transpose()?;
    let mut args: Vec<OsString> = file.iter().map(password_arg).collect();
    args.extend(["--json=2".into(), "--json-key=encrypt".into(), input.into()]);
    let v: Value = serde_json::from_str(&qpdf_out(&args)?)
        .map_err(|e| PagesError::Qpdf(format!("unreadable qpdf JSON: {e}")))?;
    // ponytail: `recovereduserpassword` in this JSON is never read or kept.
    let (e, params) = (&v["encrypt"]["capabilities"], &v["encrypt"]["parameters"]);
    let cap = |k: &str| e[k].as_bool().unwrap_or(false);
    let print = match (cap("printhigh"), cap("printlow")) {
        (true, _) => Print::High,
        (false, true) => Print::Low,
        _ => Print::None,
    };
    let method = match params["method"].as_str() {
        Some("AESv3") => "AES-256".to_string(),
        Some("AESv2") => "AES-128".to_string(),
        Some("RC4") => format!("RC4-{}", params["bits"]),
        m => m.unwrap_or("unknown").to_string(),
    };
    Ok(SecurityInfo {
        method: Some(method),
        permissions: Some(Permissions {
            print,
            modify: cap("modifyother"),
            extract: cap("extract"),
            annotate: cap("modifyannotations"),
            fill_forms: cap("modifyforms"),
            assemble: cap("modifyassembly"),
            accessibility: cap("accessibility"),
        }),
        ..locked
    })
}
