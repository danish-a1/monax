use super::*;

fn set(v: &[&str]) -> HashSet<String> {
    v.iter().map(|s| s.to_string()).collect()
}

#[test]
fn layers_are_rebuilt_from_kept_groups_and_their_states_only() {
    let d = Doc::of(json!({
        "obj:1 0 R": { "value": { "/OCProperties": { "/OCGs": ["70 0 R", "71 0 R", "72 0 R"], "/D": "9 0 R",
            "/Configs": [{ "/Name": "u:x", "/Order": ["70 0 R"] }] } } },
        "obj:9 0 R": { "value": { "/BaseState": "u:OFF", "/ON": ["71 0 R"], "/OFF": ["70 0 R", "71 0 R"],
            "/Order": ["70 0 R", ["u:x", "71 0 R", "8 0 R"]], "/Name": "u:cfg", "/X": "30 0 R",
            "/AS": [{ "/Event": "/View", "/OCGs": ["70 0 R", "71 0 R"] }] } },
        // An indirect /Order sub-array listing itself, the kept group and
        // a non-group.
        "obj:8 0 R": { "value": ["8 0 R", "70 0 R", "30 0 R", "8 0 R"] },
        "obj:30 0 R": { "value": { "/Contents": "u:secret" } },
        "obj:70 0 R": { "value": { "/Type": "/OCG", "/Name": "u:A" } },
        "obj:71 0 R": { "value": { "/Type": "/OCG", "/Name": "u:B" } },
        // Listed, typed a font, drawn by no page as a group.
        "obj:72 0 R": { "value": { "/Type": "/Font" } },
        "obj:2 0 R": { "value": { "/OCProperties": { "/OCGs": ["70 0 R", "72 0 R"], "/D": { "/BaseState": "/Secret" } } } },
    }));
    let (kept, red) = (set(&["70 0 R"]), set(&[]));
    let g = || (&kept, &red);
    let (v, gone) = layers(&d, "1 0 R", g(), &mut Map::new());
    assert!(gone, "71, a group, went");
    assert_eq!(
        v.unwrap(),
        json!({ "/OCGs": ["70 0 R"], "/D": { "/BaseState": "/OFF", "/OFF": ["70 0 R"], "/Order": ["70 0 R"] } })
    );
    assert_eq!(
        layers(&d, "2 0 R", g(), &mut Map::new()),
        (Some(json!({ "/OCGs": ["70 0 R"], "/D": {} })), false),
        "only a base state of OFF, /ON or /Unchanged is kept; the font is no group"
    );
    let empty = set(&[]);
    let nothing = (&empty, &empty);
    assert_eq!(layers(&d, "1 0 R", nothing, &mut Map::new()), (None, true));
    let nothing = (&empty, &empty);
    assert_eq!(layers(&d, "3 0 R", nothing, &mut Map::new()), (None, false));
}

#[test]
fn kept_groups_keep_type_name_and_intent_and_look_the_same_without_usage() {
    let d = Doc::of(json!({
        "obj:1 0 R": { "value": { "/OCProperties": { "/OCGs": ["70 0 R", "71 0 R", "74 0 R", "31 0 R"], "/D": "9 0 R" } } },
        "obj:9 0 R": { "value": { "/ON": ["70 0 R", "71 0 R"], "/OFF": ["74 0 R"] } },
        "obj:70 0 R": { "value": { "/Type": "/OCG", "/Name": "u:A", "/Intent": ["/View", "u:x", "u:Design", "75 0 R"],
            "/Usage": { "/CreatorInfo": { "/Creator": "u:secret" } }, "/Priv": "31 0 R" } },
        "obj:75 0 R": { "value": "/All" },
        "obj:71 0 R": { "value": { "/Type": "/OCG", "/Name": { "/T": "u:secret" }, "/Intent": "u:Design",
            "/Usage": { "/View": { "/ViewState": "/OFF" } } } },
        // Drawn with but not listed in /OCGs, off for viewing.
        "obj:72 0 R": { "value": { "/Type": "/OCG", "/Name": "u:S", "/Usage": "73 0 R", "/Priv": "u:secret" } },
        "obj:73 0 R": { "value": { "/View": { "/ViewState": "/OFF" } } },
        // Off by /D, but any view state but OFF shows it; an unknown intent;
        // untyped (pdfium takes it for a group).
        "obj:74 0 R": { "value": { "/Name": "u:T", "/Intent": "/Foo", "/Usage": { "/View": { "/ViewState": "/Foo" } } } },
        // A redacted page's font listed in /OCGs: no group, no note.
        "obj:31 0 R": { "value": { "/Type": "/Font" } },
    }));
    let (kept, red) = (set(&["70 0 R", "71 0 R", "72 0 R", "74 0 R"]), set(&[]));
    let mut up = Map::new();
    let g = (&kept, &red);
    let (v, gone) = layers(&d, "1 0 R", g, &mut up);
    assert!(!gone);
    assert_eq!(
        v.unwrap(),
        json!({ "/OCGs": ["70 0 R", "71 0 R", "74 0 R", "72 0 R"],
            "/D": { "/ON": ["70 0 R", "74 0 R"], "/OFF": ["71 0 R", "72 0 R"] } })
    );
    let got = |r: &str| up[&format!("obj:{r}")]["value"].clone();
    assert_eq!(
        got("70 0 R"),
        json!({ "/Type": "/OCG", "/Name": "u:A", "/Intent": ["/View", "/Design", "/All"] })
    );
    assert_eq!(
        got("71 0 R"),
        json!({ "/Type": "/OCG", "/Name": "u:", "/Intent": "/Design" })
    );
    assert_eq!(got("72 0 R"), json!({ "/Type": "/OCG", "/Name": "u:S" }));
    assert_eq!(
        got("74 0 R"),
        json!({ "/Type": "/OCG", "/Name": "u:T", "/Intent": [] })
    );
    assert_eq!(up.len(), 4, "only group dictionaries are rewritten");
}

#[test]
fn names_and_text_strings_read_alike() {
    let d = Doc::of(json!({
        "obj:1 0 R": { "value": { "/OCProperties": { "/OCGs": ["70 0 R", "71 0 R", "72 0 R"],
            "/D": { "/BaseState": "/Unchanged", "/ON": ["70 0 R"] } } } },
        "obj:70 0 R": { "value": { "/Type": "u:OCG", "/Name": "u:A", "/Intent": "9 0 R" } },
        "obj:9 0 R": { "value": "u:Design" },
        "obj:71 0 R": { "value": { "/Type": "/OCG", "/Name": "u:B", "/Usage": { "/View": { "/ViewState": "u:OFF" } } } },
        // Typed a text string "OCG" through a reference, drawn by nobody.
        "obj:72 0 R": { "value": { "/Type": "8 0 R" } },
        "obj:8 0 R": { "value": "u:OCG" },
    }));
    let (kept, red) = (set(&["70 0 R", "71 0 R"]), set(&[]));
    let mut up = Map::new();
    let (v, gone) = layers(&d, "1 0 R", (&kept, &red), &mut up);
    assert!(gone, "72 is a group, and it went");
    assert_eq!(up["obj:70 0 R"]["value"]["/Intent"], json!("/Design"));
    assert_eq!(
        v.unwrap()["/D"],
        json!({ "/BaseState": "/Unchanged", "/ON": ["70 0 R"], "/OFF": ["71 0 R"] })
    );
}

#[test]
fn a_long_chain_of_indirect_arrays_is_walked_and_a_null_listed_group_ignored() {
    // 100000 indirect arrays, one inside the next, then group 73.
    let n = 100_000;
    let mut o = serde_json::Map::new();
    for i in 0..n {
        let next = if i + 1 < n {
            json!([format!("{} 0 R", 1001 + i)])
        } else {
            json!(["73 0 R"])
        };
        o.insert(format!("obj:{} 0 R", 1000 + i), json!({ "value": next }));
    }
    o.insert("obj:73 0 R".into(), json!({ "value": { "/Type": "/OCG" } }));
    // /Order is flattened; /OCGs is read by its own elements, as pdfium
    // does, so 73 in a sub-array of it is not listed.
    let ocp = json!({ "/OCGs": ["73 0 R", "1000 0 R"], "/D": { "/Order": "1000 0 R" } });
    o.insert(
        "obj:1 0 R".into(),
        json!({ "value": { "/OCProperties": ocp } }),
    );
    let ocp = json!({ "/OCGs": "1000 0 R", "/D": { "/OFF": ["73 0 R"] } });
    o.insert(
        "obj:3 0 R".into(),
        json!({ "value": { "/OCProperties": ocp } }),
    );
    // Listed, but null (explicitly, or dangling).
    o.insert("obj:74 0 R".into(), json!({ "value": null }));
    let ocp = json!({ "/OCGs": ["73 0 R", "74 0 R"] });
    o.insert(
        "obj:2 0 R".into(),
        json!({ "value": { "/OCProperties": ocp } }),
    );
    let d = Doc::of(Value::Object(o));
    let (kept, red) = (set(&["73 0 R"]), set(&[]));
    let (v, _) = layers(&d, "1 0 R", (&kept, &red), &mut Map::new());
    let want = json!({ "/OCGs": ["73 0 R"], "/D": { "/Order": ["73 0 R"] } });
    assert_eq!(v.unwrap(), want);
    let (v, _) = layers(&d, "3 0 R", (&kept, &red), &mut Map::new());
    assert_eq!(v, None, "73 is in no list: visible, as in pdfium");
    let (v, _) = layers(&d, "2 0 R", (&kept, &red), &mut Map::new());
    let want = json!({ "/OCGs": ["73 0 R"], "/D": {} });
    assert_eq!(v.unwrap(), want, "a null entry is no group, as in pdfium");
}

#[test]
fn many_groups_listed_everywhere_take_linear_time() {
    let n = 60_000;
    let refs: Vec<String> = (0..n).map(|i| format!("{} 0 R", 100 + i)).collect();
    let mut o = serde_json::Map::new();
    for r in &refs {
        o.insert(format!("obj:{r}"), json!({ "value": { "/Type": "/OCG" } }));
    }
    let d = json!({ "/ON": refs, "/OFF": refs, "/Order": refs });
    let ocp = json!({ "/OCGs": refs, "/D": d });
    o.insert(
        "obj:1 0 R".into(),
        json!({ "value": { "/OCProperties": ocp } }),
    );
    let d = Doc::of(Value::Object(o));
    let kept: HashSet<String> = refs.iter().cloned().collect();
    let t = std::time::Instant::now();
    let (v, _) = layers(&d, "1 0 R", (&kept, &set(&[])), &mut Map::new());
    assert!(t.elapsed().as_secs() < 5, "{:?}", t.elapsed());
    assert_eq!(v.unwrap()["/OCGs"].as_array().unwrap().len(), n);
}
