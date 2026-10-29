use super::*;
use serde_json::json;

fn set(w: &[&str]) -> Words {
    w.iter().map(|w| w.as_bytes().to_vec()).collect()
}

#[test]
fn words_are_ascii_alphanumeric_runs_of_four_or_more() {
    assert_eq!(words(b"Top-SECRET, ok: 1234 abc"), set(&["SECRET", "1234"]));
}

#[test]
fn strings_collects_literal_and_hex_operands_joining_one_array() {
    let s = strings(
        b"/Name BT (a (nested\\) b) Tj) <4142> Tj << /K 1 >> BDC [(Re) -10 (port) <41>] TJ (x) Tj",
    );
    let s = String::from_utf8(s).unwrap();
    assert_eq!(
        s.split_whitespace().collect::<Vec<_>>(),
        ["a", "(nested)", "b)", "Tj", "AB", "ReportA", "x"]
    );
}

#[test]
fn base64_decodes_padded_and_rejects_junk() {
    assert_eq!(base64("TWFu").unwrap(), b"Man");
    assert_eq!(base64("TWE=").unwrap(), b"Ma");
    assert_eq!(base64("TQ==").unwrap(), b"M");
    assert_eq!(base64("T!=="), None);
}

#[test]
fn scan_reads_what_the_pages_draw_and_every_non_appearance_form() {
    let secret = set(&["TOPSECRET"]);
    let lit = "QlQgKFRPUFNFQ1JFVCkgVGogRVQ="; // BT (TOPSECRET) Tj ET
    let hex = "PDU0NEY1MDUzNDU0MzUyNDU1ND4gVGo="; // <544F...> Tj
    let tj = "WyhUT1ApIC0xMCAoU0VDUkVUKV0gVEo="; // [(TOP) -10 (SECRET)] TJ
    let other = "KHBsYWluIHdvcmRzKSBUag=="; // (plain words) Tj
    // Page 3 (resources inherited from 2) draws 5, form 6 (which draws form
    // 7), tiling pattern 10 and Type3 glyph 11; form 12 is drawn by nothing
    // but still searched; 8 is a ToUnicode-like stream and 9 an appearance.
    let doc = |at: &str, data: &str| {
        let mut objs = json!({
            "trailer": { "value": { "/Root": "1 0 R" } },
            "obj:1 0 R": { "value": { "/Pages": "2 0 R" } },
            "obj:2 0 R": { "value": { "/Kids": ["3 0 R"], "/Resources": { "/XObject": { "/A": "6 0 R" },
                "/Pattern": { "/P": "10 0 R" }, "/Font": { "/T3": { "/Subtype": "/Type3", "/CharProcs": { "/a": "11 0 R" } } } } } },
            "obj:3 0 R": { "value": { "/Parent": "2 0 R", "/Contents": "5 0 R",
                "/Annots": [{ "/AP": { "/N": "9 0 R" } }] } },
            "obj:5 0 R": { "stream": { "dict": {}, "data": other } },
            "obj:6 0 R": { "stream": { "dict": { "/Subtype": "/Form", "/Resources": { "/XObject": { "/B": "7 0 R" } } }, "data": other } },
            "obj:7 0 R": { "stream": { "dict": { "/Subtype": "/Form" }, "data": other } },
            "obj:8 0 R": { "stream": { "dict": {}, "data": other } },
            "obj:9 0 R": { "stream": { "dict": { "/Subtype": "/Form" }, "data": other } },
            "obj:10 0 R": { "stream": { "dict": { "/PatternType": 1 }, "data": other } },
            "obj:11 0 R": { "stream": { "dict": {}, "data": other } },
            "obj:12 0 R": { "stream": { "dict": { "/Subtype": "/Form" }, "data": other } },
        });
        objs[format!("obj:{at} 0 R")]["stream"]["data"] = json!(data);
        Doc::of(objs)
    };
    for at in ["5", "6", "7", "10", "11", "12"] {
        for data in [lit, hex, tj] {
            let r = scan(&doc(at, data), &secret);
            assert!(matches!(r, Err(PagesError::RedactionLeak)), "{at}: {data}");
        }
    }
    for at in ["8", "9"] {
        assert!(scan(&doc(at, lit), &secret).is_ok(), "{at} is not drawn");
    }
    assert!(scan(&doc("5", other), &secret).is_ok());
}

#[test]
fn drawn_ends_type3_and_resource_cycles_and_spares_forms_appearances_draw() {
    // Type3 font 4 names itself and its glyph's resources (6) name it
    // again; appearance 7 draws form 8, which draws page 3's form 9.
    let d = Doc::of(json!({
        "trailer": { "value": { "/Root": "1 0 R" } },
        "obj:1 0 R": { "value": { "/Pages": "2 0 R" } },
        "obj:2 0 R": { "value": { "/Kids": ["3 0 R"] } },
        "obj:3 0 R": { "value": { "/Resources": { "/Font": { "/T": "4 0 R" }, "/XObject": { "/F": "9 0 R" } },
            "/Annots": [{ "/AP": { "/N": "7 0 R" } }] } },
        "obj:4 0 R": { "value": { "/Subtype": "/Type3", "/CharProcs": { "/a": "5 0 R" }, "/Resources": "6 0 R" } },
        "obj:5 0 R": { "stream": { "dict": {} } },
        "obj:6 0 R": { "value": { "/Font": { "/T": "4 0 R", "/U": { "/Subtype": "/Type3", "/Resources": "6 0 R" } } } },
        "obj:7 0 R": { "stream": { "dict": { "/Subtype": "/Form", "/Resources": { "/XObject": { "/N": "8 0 R" } } } } },
        "obj:8 0 R": { "stream": { "dict": { "/Subtype": "/Form", "/Resources": { "/XObject": { "/P": "9 0 R" } } } } },
        "obj:9 0 R": { "stream": { "dict": { "/Subtype": "/Form" } } },
    }));
    let mut got: Vec<String> = scanned(&d).into_iter().collect();
    got.sort();
    assert_eq!(got, ["5 0 R", "9 0 R"], "8 is an appearance's own form");
}
