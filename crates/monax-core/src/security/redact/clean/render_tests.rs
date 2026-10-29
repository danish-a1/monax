use super::*;
use serde_json::json;

#[test]
fn only_what_draws_is_reached_and_groups_are_found_as_pdfium_does() {
    let d = Doc::of(json!({
        "obj:3 0 R": { "value": { "/Resources": "20 0 R", "/Contents": "5 0 R", "/Annots": ["30 0 R"],
            "/PieceInfo": "90 0 R", "/MediaBox": "6 0 R" } },
        "obj:5 0 R": { "stream": { "dict": { "/Length": "7 0 R", "/Priv": "91 0 R" } } },
        "obj:6 0 R": { "value": [0, 0, 200, 200] },
        "obj:7 0 R": { "value": 40 },
        "obj:20 0 R": { "value": { "/Font": { "/F1": "21 0 R" }, "/Properties": {
            "/a": "70 0 R", "/b": "71 0 R", "/c": "72 0 R", "/d": "73 0 R" } } },
        "obj:21 0 R": { "value": { "/Type": "/Font", "/FontDescriptor": "22 0 R", "/Priv": "92 0 R", "/Name": "96 0 R", "/Encoding": "97 0 R", "/Widths": "5 0 R" } },
        // Plain data never enters a dictionary; an /Encoding's /Differences
        // draws.
        "obj:96 0 R": { "value": { "/Private": "u:x" } },
        "obj:97 0 R": { "value": { "/Differences": "98 0 R", "/BaseEncoding": "/WinAnsiEncoding" } },
        "obj:98 0 R": { "value": [65, "/W"] },
        "obj:22 0 R": { "value": { "/FontFile2": "23 0 R", "/Ascent": 700 } },
        "obj:23 0 R": { "stream": { "dict": { "/Length1": 10 } } },
        "obj:30 0 R": { "value": { "/Subtype": "/Widget", "/AP": { "/N": { "/On": "31 0 R" } }, "/A": "93 0 R",
            "/Parent": "40 0 R", "/OC": "74 0 R" } },
        "obj:31 0 R": { "stream": { "dict": { "/Resources": { "/Font": { "/F1": "21 0 R" } } } } },
        "obj:40 0 R": { "value": { "/V": "u:x", "/AA": "94 0 R" } },
        // An untyped group (/Priv is never followed), an OCMD typed /Foo
        // with a member typed /Foo, a group typed through a reference.
        "obj:70 0 R": { "value": { "/Name": "u:L", "/Priv": "95 0 R" } },
        "obj:71 0 R": { "value": { "/Type": "/Foo", "/OCGs": "75 0 R", "/VE": ["/Not", "76 0 R"] } },
        "obj:75 0 R": { "value": { "/Type": "/Foo" } },
        "obj:76 0 R": { "value": ["/Or", "77 0 R"] },
        "obj:77 0 R": { "value": {} },
        "obj:72 0 R": { "value": { "/Type": "78 0 R" } },
        "obj:78 0 R": { "value": "/OCG" },
        "obj:73 0 R": { "value": { "/Type": "/OCG" } },
        "obj:74 0 R": { "value": { "/Type": "/OCG" } },
    }));
    let (drawn, groups) = render(&d, vec![(&json!("3 0 R"), K::Page)], &HashSet::new());
    let sorted = |s: HashSet<String>| {
        let mut v: Vec<String> = s.into_iter().collect();
        v.sort();
        v
    };
    assert_eq!(
        sorted(groups),
        ["70 0 R", "72 0 R", "73 0 R", "74 0 R", "75 0 R", "77 0 R"]
    );
    let drawn = sorted(drawn);
    for r in [
        "90 0 R", "91 0 R", "92 0 R", "93 0 R", "94 0 R", "95 0 R", "78 0 R", "96 0 R",
    ] {
        assert!(!drawn.contains(&r.to_string()), "{r} is not drawn with");
    }
    for r in [
        "5 0 R", "6 0 R", "7 0 R", "21 0 R", "22 0 R", "23 0 R", "31 0 R", "40 0 R", "76 0 R",
        "97 0 R", "98 0 R",
    ] {
        assert!(drawn.contains(&r.to_string()), "{r} draws");
    }
}

#[test]
fn border_and_cid_info_dictionaries_draw_through_their_plain_keys_only() {
    let d = Doc::of(json!({
        "obj:3 0 R": { "value": { "/Resources": { "/Font": { "/F1": "21 0 R" } }, "/Annots": ["30 0 R", "31 0 R"] } },
        "obj:30 0 R": { "value": { "/Subtype": "/Square", "/BS": "50 0 R", "/BE": "51 0 R" } },
        // A /BS that is a stream (page 1's content) is never drawn with.
        "obj:31 0 R": { "value": { "/Subtype": "/Square", "/BS": "52 0 R" } },
        "obj:50 0 R": { "value": { "/W": "53 0 R", "/S": "/D", "/D": [3], "/Priv": "90 0 R" } },
        "obj:51 0 R": { "value": { "/S": "/C", "/I": 1 } },
        "obj:52 0 R": { "stream": { "dict": { "/Length": 9 } } },
        "obj:53 0 R": { "value": 12 },
        "obj:21 0 R": { "value": { "/Subtype": "/Type0", "/DescendantFonts": ["22 0 R"] } },
        "obj:22 0 R": { "value": { "/CIDSystemInfo": "23 0 R" } },
        "obj:23 0 R": { "value": { "/Registry": "u:Adobe", "/Ordering": "24 0 R", "/Supplement": "25 0 R",
            "/Priv": "91 0 R" } },
        "obj:24 0 R": { "value": "u:Japan1" },
        "obj:25 0 R": { "value": 4 },
    }));
    let (drawn, _) = render(&d, vec![(&json!("3 0 R"), K::Page)], &HashSet::new());
    for r in ["50 0 R", "51 0 R", "53 0 R", "23 0 R", "24 0 R", "25 0 R"] {
        assert!(drawn.contains(r), "{r} draws");
    }
    for r in ["52 0 R", "90 0 R", "91 0 R"] {
        assert!(!drawn.contains(r), "{r} is not drawn with");
    }
}
