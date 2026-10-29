//! A checkbox whose normal appearance is one stream, not a state
//! dictionary: the stream's dictionary keys are no states.
use super::*;

#[test]
fn a_stream_normal_appearance_has_no_on_state() {
    let box_ = json!({ "/T": "u:box", "/FT": "/Btn", "/AS": "/BBox", "/AP": { "/N": "31 0 R" } });
    let mut d = doc(json!({ "/Fields": ["10 0 R"] }), &["10 0 R"], &[(10, box_)]);
    let stream = json!({ "stream": { "dict": { "/BBox": [0, 0, 20, 20] } } });
    d.objects.insert("obj:31 0 R".into(), stream);
    let n = &nodes(&d)[0];
    assert!(n.field.options.is_empty(), "{:?}", n.field.options);
    for v in ["BBox", "true"] {
        let e = Plan::new(&d).set(n, v).unwrap_err().to_string();
        let want = "invalid input: the value for field \"box\" is not one of its values (Off)";
        assert_eq!(e, want);
    }
    let e = drawn(&d, &HashSet::from(["box"])).unwrap_err().to_string();
    assert!(
        e.starts_with("form field \"box\" could not be drawn"),
        "{e}"
    );
}
