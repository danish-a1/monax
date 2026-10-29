use super::*;
use crate::pdfium::PdfiumError;

fn doc() -> Doc {
    Doc::of(json!({
        "trailer": { "value": { "/Root": "1 0 R" } },
        "obj:1 0 R": { "value": { "/Type": "/Catalog", "/Pages": "2 0 R" } },
        "obj:2 0 R": { "value": { "/Type": "/Pages", "/Kids": ["3 0 R", "4 0 R", "5 0 R"] } },
        "obj:3 0 R": { "value": { "/Type": "/Page", "/Rotate": 90 } },
        "obj:4 0 R": { "value": { "/Type": "/Page", "/Annots": "6 0 R" } },
        "obj:5 0 R": { "value": { "/Type": "/Page", "/Annots": 7 } },
        "obj:6 0 R": { "value": ["9 0 R"] },
    }))
}

fn built(tag: &str) -> Built {
    let mut dict = Map::new();
    dict.insert("/Subtype".into(), json!(tag));
    Built {
        dict,
        form: json!({ "/Subtype": "/Form" }),
        content: b"q Q".to_vec(),
    }
}

#[test]
fn new_objects_follow_the_highest_id_and_pages_list_them() {
    let d = doc();
    let first = d.new_id();
    let up = updates(
        &d,
        3,
        vec![
            (1, built("/Ink")),
            (0, built("/Square")),
            (1, built("/Text")),
        ],
    )
    .unwrap();
    let r = |k: u64| format!("{} 0 R", first + k);
    let annot = &up[&format!("obj:{}", r(0))]["value"];
    assert_eq!(annot["/Subtype"], "/Ink");
    assert_eq!(annot["/P"], "4 0 R");
    assert_eq!(annot["/AP"], json!({ "/N": r(1) }));
    let ap = &up[&format!("obj:{}", r(1))]["stream"];
    assert_eq!(
        (&ap["dict"], &ap["data"]),
        (&json!({ "/Subtype": "/Form" }), &json!("cSBR"))
    );
    // Page 1's indirect /Annots is copied into a direct array; the array
    // object itself is not rewritten.
    let p1 = &up["obj:4 0 R"]["value"];
    assert_eq!(p1["/Annots"], json!(["9 0 R", r(0), r(4)]));
    assert_eq!(p1["/Type"], "/Page");
    assert!(!up.contains_key("obj:6 0 R"));
    let p0 = &up["obj:3 0 R"]["value"];
    assert_eq!(
        (&p0["/Annots"], &p0["/Rotate"]),
        (&json!([r(2)]), &json!(90))
    );
    assert_eq!(up.len(), 2 * 3 + 2);
}

#[test]
fn bad_annots_is_damaged() {
    let d = doc();
    assert!(matches!(
        updates(&d, 3, vec![(2, built("/Ink"))]),
        Err(PagesError::Damaged)
    ));
}

fn repair_first(r: Result<Map<String, Value>, PagesError>) -> bool {
    matches!(r, Err(PagesError::Pdfium(PdfiumError::DamagedRepairFirst)))
}

#[test]
fn a_page_tree_pdfium_counts_otherwise_is_refused() {
    // pdfium sees a fourth page qpdf's page tree does not list.
    assert!(repair_first(updates(&doc(), 4, vec![(0, built("/Ink"))])));
    // A page listed twice: qpdf's tree drops the repeat, pdfium counts it.
    let mut twice = doc();
    let kids = json!({ "/Type": "/Pages", "/Kids": ["3 0 R", "4 0 R", "3 0 R"] });
    twice.objects["obj:2 0 R"] = json!({ "value": kids });
    assert!(repair_first(updates(&twice, 3, vec![(0, built("/Ink"))])));
}
