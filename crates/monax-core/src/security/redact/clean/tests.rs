use super::*;

fn doc() -> Doc {
    Doc::of(json!({
        "trailer": { "value": { "/Root": "1 0 R" } },
        "obj:1 0 R": { "value": { "/Pages": "2 0 R", "/StructTreeRoot": "9 0 R",
            "/AcroForm": { "/Fields": ["20 0 R", "7 0 R"], "/CO": ["20 0 R", "7 0 R"] } } },
        "obj:2 0 R": { "value": { "/Kids": ["3 0 R", "4 0 R"] } },
        "obj:3 0 R": { "value": { "/Contents": "12 0 R", "/Annots": ["5 0 R", "7 0 R", "21 0 R"] } },
        "obj:4 0 R": { "value": { "/Contents": ["13 0 R", "12 0 R"], "/Annots": ["6 0 R", "22 0 R", "5 0 R"] } },
        "obj:5 0 R": { "value": { "/Subtype": "/Link", "/Dest": ["4 0 R", "/Fit"] } },
        "obj:6 0 R": { "value": { "/Subtype": "/Text", "/Contents": "u:secret" } },
        "obj:7 0 R": { "value": { "/Subtype": "/Widget" } },
        // A radio field with one widget per page; the redacted one is on.
        "obj:20 0 R": { "value": { "/FT": "/Btn", "/V": "/On2", "/Kids": ["21 0 R", "22 0 R"] } },
        "obj:21 0 R": { "value": { "/Subtype": "/Widget", "/AP": { "/N": { "/On1": "30 0 R", "/Off": "30 0 R" } } } },
        "obj:22 0 R": { "value": { "/Subtype": "/Widget", "/AP": { "/N": { "/On2": "30 0 R", "/Off": "30 0 R" } } } },
        "obj:11 0 R": { "value": { "/Title": "u:x", "/Dest": ["4 0 R", "/Fit"] } },
    }))
}

#[test]
fn page_refs_follow_the_tree_and_contents_come_as_a_list() {
    let d = doc();
    assert_eq!(page_refs(&d), ["3 0 R", "4 0 R"]);
    assert_eq!(contents(&d, "4 0 R"), [json!("13 0 R"), json!("12 0 R")]);
    assert_eq!(contents(&d, "3 0 R"), [json!("12 0 R")]);
}

#[test]
fn what_only_a_redacted_page_lists_is_nulled_and_leaves_its_field() {
    let (up, notes) = plan(&doc(), &[1]).unwrap();
    let keys: Vec<&str> = up.keys().map(String::as_str).collect();
    // 6, 22 (annotations) and 13 (content) only on page 1; 5 and 12 shared;
    // bookmark 11 describes page 1; link 5 (listed on page 0) stays but
    // loses its /Dest; the trailer loses /Info.
    assert_eq!(
        keys,
        [
            "obj:1 0 R",
            "obj:11 0 R",
            "obj:13 0 R",
            "obj:20 0 R",
            "obj:22 0 R",
            "obj:5 0 R",
            "obj:6 0 R",
            "trailer"
        ]
    );
    assert_eq!(up["obj:5 0 R"]["value"]["/Dest"], Value::Null);
    assert_eq!(up["obj:5 0 R"]["value"]["/Subtype"], "/Link");
    for k in ["obj:11 0 R", "obj:13 0 R", "obj:22 0 R", "obj:6 0 R"] {
        assert_eq!(up[k], json!({ "value": null }), "{k}");
    }
    let cat = &up["obj:1 0 R"]["value"];
    let keys: Vec<&String> = cat.as_object().unwrap().keys().collect();
    assert_eq!(
        keys,
        ["/AcroForm", "/Pages", "/Type"],
        "only pages and form"
    );
    let field = &up["obj:20 0 R"]["value"];
    assert_eq!(
        (&field["/Kids"], &field["/V"]),
        (&json!(["21 0 R"]), &json!("/Off"))
    );
    // Kept page 0 links to page 1.
    assert_eq!(notes, [TAGS, DESTS, SHARED]);
}

#[test]
fn a_field_with_no_widget_left_goes_and_a_short_tree_is_damage() {
    let (up, notes) = plan(&doc(), &[0]).unwrap();
    // Page 0 alone lists 7 (a field-widget) and 21: 7 leaves /Fields and /CO.
    let form = &up["obj:1 0 R"]["value"]["/AcroForm"];
    assert_eq!(
        (&form["/Fields"], &form["/CO"]),
        (&json!(["20 0 R"]), &json!(["20 0 R"]))
    );
    assert_eq!(up["obj:7 0 R"], json!({ "value": null }));
    assert_eq!(
        up["obj:20 0 R"]["value"]["/V"], "/On2",
        "its state is still shown"
    );
    assert_eq!(notes, [TAGS, SHARED], "the link is to kept page 1");
    assert!(matches!(plan(&doc(), &[2]), Err(PagesError::Damaged)));
}

#[test]
fn owned_follows_every_key_and_stops_only_at_pages() {
    let d = Doc::of(json!({
        "trailer": { "value": { "/Root": "1 0 R" } },
        "obj:1 0 R": { "value": { "/Pages": "2 0 R", "/Foo": "40 0 R",
            "/AcroForm": { "/DR": { "/Font": { "/H": "42 0 R" } } } } },
        "obj:2 0 R": { "value": { "/Type": "/Pages", "/Kids": ["3 0 R", "4 0 R"], "/Resources": { "/Font": { "/S": "41 0 R" } } } },
        "obj:3 0 R": { "value": { "/Parent": "2 0 R" } },
        // Page 1 inherits font 41 (shared), owns 40 (also named by the
        // catalog), 42 (also the form's /DR: no document root keeps it), 45
        // (behind a private /P key) and annotation 43 with its /Parent 44;
        // 43's /P and a /Dest to page 0 stop at the pages.
        "obj:4 0 R": { "value": { "/Parent": "2 0 R", "/PieceInfo": "40 0 R", "/Thumb": "42 0 R",
            "/Annots": ["43 0 R"], "/X": { "/Private": { "/P": "45 0 R" } } } },
        "obj:43 0 R": { "value": { "/P": "4 0 R", "/Dest": ["3 0 R", "/Fit"], "/Parent": "44 0 R" } },
    }));
    let (pages, nodes) = d.page_tree();
    let stop = &(&nodes | &pages.iter().cloned().collect()) | &HashSet::from(["1 0 R".into()]);
    let r = owned::owned(&d, &pages, &[1], &stop, &HashSet::new());
    let mut got: Vec<String> = r.owned.into_iter().collect();
    got.sort();
    assert_eq!(got, ["40 0 R", "42 0 R", "43 0 R", "44 0 R", "45 0 R"]);
    assert_eq!(r.kept, HashSet::from(["41 0 R".to_string()]));
}

#[test]
fn dropped_names_features_once_and_unknown_keys_as_they_are() {
    let d = Doc::of(json!({
        "trailer": { "value": { "/Root": "1 0 R", "/Info": "9 0 R" } },
        "obj:5 0 R": { "value": { "/Type": "/Outlines" } },
    }));
    let cat = json!({ "/Pages": "2 0 R", "/Outlines": "5 0 R", "/Metadata": "8 0 R", "/Lang": "u:en",
        "/PageMode": "/UseOutlines", "/PageLayout": "/OneColumn", "/MarkInfo": {},
        "/Names": { "/Dests": "6 0 R", "/EmbeddedFiles": "7 0 R", "/Templates": "10 0 R" } });
    let form = json!({ "/XFA": "11 0 R", "/CO": [] });
    assert_eq!(
        dropped::dropped(&d, &cat, &form),
        [
            "named destinations",
            "attachments",
            "metadata",
            "viewer settings",
            "XFA form data",
            "/Lang",
            "/Names /Templates"
        ],
        "an outline tree without items and an empty /CO lose nothing"
    );
}

#[test]
fn cut_nulls_what_describes_a_redacted_page_and_rewrites_the_rest() {
    let red = HashSet::from(["4 0 R".to_string()]);
    let annots = json!(["71 0 R", { "/Subtype": "/Link", "/Dest": ["4 0 R", "/Fit"],
        "/A": { "/S": "/GoTo", "/D": ["4 0 R"] }, "/BS": { "/S": "/D" } }, "4 0 R"]);
    assert_eq!(
        cut::cut(&annots, &red),
        json!(["71 0 R", { "/Subtype": "/Link", "/Dest": null, "/A": null, "/BS": { "/S": "/D" } }, null])
    );
    let form = json!({ "/Subtype": "/Form", "/Pg": "4 0 R" });
    assert!(
        !cut::describes(&form, &red),
        "a form naming the page is no describer"
    );
    for d in [
        json!({ "/S": "/P", "/Pg": "4 0 R" }),
        json!({ "/Title": "u:x", "/Dest": ["4 0 R"] }),
        json!({ "/Type": "/OBJR", "/Pg": "4 0 R" }),
    ] {
        assert!(cut::describes(&d, &red), "{d}");
    }
}

#[test]
fn a_field_kids_dag_is_decided_once_per_field() {
    // 30 fields, each listing the next twice (2^30 paths) and a widget only
    // a redacted page listed, then a kept widget: each field is rewritten
    // once without the dead widget, in linear time.
    let mut o = serde_json::Map::new();
    for i in 100..130 {
        let k = format!("{} 0 R", i + 1);
        o.insert(
            format!("obj:{i} 0 R"),
            json!({ "value": { "/Kids": [k, k, "200 0 R"] } }),
        );
    }
    o.insert(
        "obj:130 0 R".into(),
        json!({ "value": { "/Subtype": "/Widget" } }),
    );
    o.insert(
        "obj:200 0 R".into(),
        json!({ "value": { "/Subtype": "/Widget" } }),
    );
    let d = Doc::of(Value::Object(o));
    let mut dead: HashSet<String> = ["200 0 R".to_string()].into();
    let form = json!({ "/Fields": ["100 0 R"] });
    let mut up = Map::new();
    assert_eq!(fields::prune(&d, &form, &mut dead, &mut up), None);
    assert_eq!((up.len(), dead.len()), (30, 1));
    let k = &up["obj:129 0 R"]["value"]["/Kids"];
    assert_eq!(k, &json!(["130 0 R", "130 0 R"]));
}
