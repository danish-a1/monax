use monax_core::{JobState, NameCtx, Registry, names};

fn ctx(filename: &str, bookmark: Option<&str>) -> NameCtx {
    NameCtx {
        filename: filename.into(),
        page: 3,
        bookmark: bookmark.map(Into::into),
        index: 7,
        date: "2026-10-27".into(),
    }
}

#[test]
fn expands_all_placeholders() {
    let c = ctx("report", Some("Intro"));
    let out = names::expand("%[filename]_%[page]_%[bookmark]_%[date]_%[index].pdf", &c);
    assert_eq!(out.to_string_lossy(), "report_3_Intro_2026-10-27_7.pdf");
}

#[test]
fn sanitizes_values_and_result() {
    let out = names::expand("%[filename].pdf", &ctx("a/b:c\\d\0e", None));
    assert_eq!(out.to_string_lossy(), "a_b_c_d_e.pdf");
    let out = names::expand("x/%[bookmark]y", &ctx("f", None));
    assert_eq!(out.to_string_lossy(), "x_y");
}

#[test]
fn degenerate_names_become_untitled() {
    for p in ["", ".", "..", "%[bookmark]"] {
        assert_eq!(
            names::expand(p, &ctx("f", None)).to_string_lossy(),
            "untitled"
        );
    }
}

#[test]
fn never_overwrites() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("x.pdf");
    assert_eq!(names::unique_path(p.clone()), p);
    std::fs::write(&p, b"1").unwrap();
    std::fs::write(dir.path().join("x-1.pdf"), b"2").unwrap();
    assert_eq!(names::unique_path(p), dir.path().join("x-2.pdf"));
}

#[test]
fn unique_path_without_extension() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("noext");
    std::fs::write(&p, b"1").unwrap();
    assert_eq!(names::unique_path(p), dir.path().join("noext-1"));
}

#[test]
fn job_runs_and_reports_done() {
    let reg = Registry::new();
    let id = reg.submit(|_progress| Ok::<_, String>(vec!["/tmp/o.pdf".into()]));
    let job = reg.wait(id).unwrap();
    assert_eq!(job.state, JobState::Done);
    assert_eq!(job.outputs, vec![std::path::PathBuf::from("/tmp/o.pdf")]);
    assert_eq!(job.progress, 1.0);
}

#[test]
fn every_job_state_serializes_to_its_documented_json() {
    use serde_json::json;
    let cases = [
        (JobState::Queued, json!({"kind": "queued"})),
        (JobState::Running, json!({"kind": "running"})),
        (JobState::Done, json!({"kind": "done"})),
        (JobState::Cancelled, json!({"kind": "cancelled"})),
        (
            JobState::Failed("boom".into()),
            json!({"kind": "failed", "message": "boom"}),
        ),
    ];
    for (state, want) in cases {
        assert_eq!(serde_json::to_value(&state).unwrap(), want);
    }
}

#[test]
fn job_id_is_a_string_and_rejects_non_numeric() {
    use monax_core::JobId;
    assert_eq!(serde_json::to_string(&JobId(42)).unwrap(), "\"42\"");
    assert_eq!(serde_json::from_str::<JobId>("\"42\"").unwrap(), JobId(42));
    assert!(serde_json::from_str::<JobId>("\"x\"").is_err());
    assert!(serde_json::from_str::<JobId>("42").is_err());
}

#[test]
fn job_and_id_schemas() {
    use monax_core::{Job, JobId};
    let id = schemars::schema_for!(JobId);
    assert_eq!(id.as_value()["type"], "string");
    let job = schemars::schema_for!(Job);
    assert!(job.as_value()["properties"]["state"].is_object());
}

#[test]
fn notes_are_kept_on_the_job_and_omitted_from_json_when_empty() {
    let reg = Registry::new();
    let plain = reg.wait(reg.submit(|_| Ok::<_, String>(vec![]))).unwrap();
    let v = serde_json::to_value(&plain).unwrap();
    assert!(v.get("notes").is_none(), "{v}");
    let id = reg.submit(|sink| {
        sink.note("2 images left above 72 dpi: color space not gray or RGB");
        sink.note(String::from("second"));
        Ok::<_, String>(vec![])
    });
    let job = reg.wait(id).unwrap();
    let want = [
        "2 images left above 72 dpi: color space not gray or RGB",
        "second",
    ];
    assert_eq!(job.notes, want);
    assert_eq!(
        serde_json::to_value(&job).unwrap()["notes"],
        serde_json::json!(want)
    );
    let schema = schemars::schema_for!(monax_core::Job);
    assert_eq!(schema.as_value()["properties"]["notes"]["type"], "array");
}
