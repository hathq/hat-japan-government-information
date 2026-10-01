use hat_japan_government_information::{
    GovernmentInformationRequest, GovernmentTopic, INPUT_SCHEMA, OFFICIAL_SOURCES, PACKAGE_ID,
    PACKAGE_JSON, REPOSITORY_ID, prepare_request,
};

const SCHEMAS: &[(&str, &str)] = &[
    (
        "hathq://hat-japan-government-information/government-information-request-input/v1",
        include_str!("../schemas/government-information-request-input-v1.schema.json"),
    ),
    (
        "hathq://hat-japan-government-information/government-information-request-output/v1",
        include_str!("../schemas/government-information-request-output-v1.schema.json"),
    ),
    (
        "hathq://hat-japan-government-information/government-information-request-event/v1",
        include_str!("../schemas/government-information-request-event-v1.schema.json"),
    ),
    (
        "hathq://hat-japan-government-information/government-information-request-projection/v1",
        include_str!("../schemas/government-information-request-projection-v1.schema.json"),
    ),
    (
        "hathq://hat-japan-government-information/official-government-source/v1",
        include_str!("../schemas/official-government-source-v1.schema.json"),
    ),
];

fn request(topic: GovernmentTopic) -> GovernmentInformationRequest {
    GovernmentInformationRequest {
        schema: INPUT_SCHEMA.into(),
        jurisdiction: "JP".into(),
        topic,
        question: "Which official source contains the applicable text?".into(),
        administrative_area_code: Some("JP-13".into()),
        as_of_date: Some("2026-09-02".into()),
    }
}

#[test]
fn package_is_exact_fail_closed_and_source_preserving() {
    let package: serde_json::Value = serde_json::from_str(PACKAGE_JSON).expect("package JSON");
    assert_eq!(package["package_id"], PACKAGE_ID);
    assert_eq!(package["repository_id"], REPOSITORY_ID);
    assert_eq!(
        package["operations"][0]["context_plan"]["unresolved_policy"],
        "record-unresolved"
    );
    assert_eq!(
        package["operations"][0]["reducer"]["conflict_policy"],
        "reject"
    );
    assert_eq!(OFFICIAL_SOURCES.len(), 4);
    assert!(
        OFFICIAL_SOURCES
            .iter()
            .all(|source| source.origin.starts_with("https://"))
    );
}

#[test]
fn exact_request_becomes_a_bounded_official_source_plan() {
    let output = prepare_request(request(GovernmentTopic::Laws)).expect("prepared plan");
    assert_eq!(output.status, "prepared");
    assert_eq!(output.sources.len(), 1);
    assert_eq!(output.sources[0].id, "e-gov-laws");
    assert!(output.unresolved_fields.is_empty());
    assert!(output.constraints.contains(&"no-legal-interpretation"));
    assert!(output.constraints.contains(&"do-not-fill-unresolved"));
}

#[test]
fn missing_scope_is_reported_and_invalid_scope_is_never_guessed() {
    let mut unresolved = request(GovernmentTopic::Administration);
    unresolved.administrative_area_code = None;
    unresolved.as_of_date = None;
    let output = prepare_request(unresolved).expect("unresolved plan remains representable");
    assert_eq!(output.status, "needs-user-input");
    assert_eq!(
        output.unresolved_fields,
        ["as-of-date", "administrative-area"]
    );

    let mut invalid = request(GovernmentTopic::Tax);
    invalid.administrative_area_code = Some("Tokyo".into());
    assert_eq!(
        prepare_request(invalid),
        Err("government-information-request-invalid")
    );
    let mut impossible_date = request(GovernmentTopic::Statistics);
    impossible_date.as_of_date = Some("2026-02-30".into());
    assert_eq!(
        prepare_request(impossible_date),
        Err("government-information-request-invalid")
    );
}

#[test]
fn released_schemas_are_closed_and_identity_bound() {
    for (identity, source) in SCHEMAS {
        let schema: serde_json::Value = serde_json::from_str(source).expect("schema JSON");
        assert_eq!(schema["$id"], *identity);
        assert_eq!(schema["additionalProperties"], false);
    }
}
