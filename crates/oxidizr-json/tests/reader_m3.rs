//! Reader acceptance tests (P1-M3b, Category A):
//!
//! * every `testdata/golden/*.json` must read and re-write BYTE-IDENTICAL
//!   (the reference `JsonReader`+`JsonWriter` round-trip contract, and the
//!   proof that the read defaults in WRITE-CATALOG §6 are the reference's);
//! * `testdata/read-corpus/` (vendored back-compat workspaces, v5-era keys)
//!   must load through the leniency matrix without panicking;
//! * `testdata/validation/` fixtures must match the reference
//!   `WorkspaceRulesValidationTests` exactly — 15 throw `JsonError::Validation`
//!   with the reference's message, 1 (per-environment uniqueness) loads.
//! * inline probes pin the Jackson binding rules §6 discovered live against
//!   the reference (2026-11-14): `instances` validation, health-check
//!   name/url nullability, the `rankDirection` guard, color normalization,
//!   the `enterpriseContextViews` replacement and `theme` aliasing.

use std::path::{Path, PathBuf};

use oxidizr_core::{User, Workspace};
use oxidizr_json::{JsonError, from_slice, to_vec};

const GOLDEN_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata/golden");
const CORPUS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata/read-corpus");
const VALIDATION_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../testdata/validation");

/// Workspace goldens are the TOP-LEVEL `testdata/golden/*.json` files —
/// the subdirectories are the diagram corpora (`plantuml[-c4]/`, `mermaid/`)
/// and the theme corpus (`theme/`, M4-d), none of which are workspaces.
fn golden_json_files() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(GOLDEN_DIR)
        .expect("golden dir readable")
        .map(|e| e.expect("dir entry").path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "json"))
        .collect();
    out.sort();
    out
}

fn json_files(dir: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![PathBuf::from(dir)];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).expect("corpus dir readable") {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "json") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

fn read_file(path: &Path) -> Vec<u8> {
    std::fs::read(path).expect("fixture readable")
}

#[test]
fn goldens_round_trip_byte_identical() {
    let files = golden_json_files();
    assert!(!files.is_empty(), "golden corpus present");
    for path in files {
        let bytes = read_file(&path);
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let workspace =
            from_slice(&bytes).unwrap_or_else(|e| panic!("{name} must load as a workspace: {e}"));
        let rewritten = to_vec(&workspace).expect("golden must re-write");
        assert_eq!(
            rewritten,
            bytes,
            "{} must round-trip byte-identically ({} vs {} bytes; first diff at {})",
            name,
            bytes.len(),
            rewritten.len(),
            bytes
                .iter()
                .zip(rewritten.iter())
                .position(|(a, b)| a != b)
                .unwrap_or_else(|| bytes.len().min(rewritten.len())),
        );
    }
}

#[test]
fn goldens_are_model_stable_under_repeated_reads() {
    // read → write → read must land on an equal model (no drift through the
    // defaults/derivation paths — the property that makes JSON↔JSON lossless).
    for path in golden_json_files() {
        let first: Workspace = from_slice(&read_file(&path)).expect("golden loads");
        let second: Workspace = from_slice(&to_vec(&first).unwrap()).expect("re-write loads");
        assert_eq!(
            first,
            second,
            "{}: model drifts across a read-write-read cycle",
            path.file_name().unwrap().to_string_lossy()
        );
    }
}

#[test]
fn read_corpus_loads_leniently() {
    // Vendored older-format workspaces (`enterprise`, `location`, …): every
    // unknown key must be ignored, and the files must hydrate (they were
    // valid workspaces when written).
    let files = json_files(CORPUS_DIR);
    assert_eq!(files.len(), 5, "vendored back-compat corpus");
    for path in &files {
        from_slice(&read_file(path))
            .unwrap_or_else(|e| panic!("{} must load: {e}", path.display()));
    }
}

/// Per-fixture expectations from the reference `WorkspaceRulesValidationTests`
/// (structurizr-client): the exact first exception message, or `None` for the
/// one fixture the reference test names
/// `exceptionNotThrown_WhenTopLevelDeploymentNodeNamesAreNotUniqueButTheyExistInDifferentEnvironments`
/// — top-level deployment node uniqueness is per (name, ENVIRONMENT) pair, so
/// that workspace hydrates (vendoring erratum: it sits in the validation
/// directory but is a positive case, confirmed against the Java tests
/// 2026-11-14).
const VALIDATION_EXPECTATIONS: &[(&str, Option<&str>)] = &[
    (
        "ChildDeploymentNodeNamesAreNotUnique.json",
        Some("A deployment node named \"Child\" already exists within \"Deployment Node\"."),
    ),
    (
        "ComponentNamesAreNotUnique.json",
        Some("A component named \"Component\" already exists within \"Container\"."),
    ),
    (
        "ContainerAssociatedWithComponentViewIsMissingFromTheModel.json",
        Some(
            "The component view with key \"Components\" is associated with a container (id=3), but that element does not exist in the model.",
        ),
    ),
    (
        "ContainerNamesAreNotUnique.json",
        Some("A container named \"Container\" already exists within \"Software System\"."),
    ),
    (
        "ElementAssociatedWithDynamicViewIsMissingFromTheModel.json",
        Some(
            "The dynamic view with key \"Dynamic\" is associated with an element (id=2), but that element does not exist in the model.",
        ),
    ),
    (
        "ElementReferencedByViewIsMissingFromTheModel.json",
        Some(
            "The view with key \"SystemLandscape\" references an element (id=2), but that element does not exist in the model.",
        ),
    ),
    (
        "PeopleAndSoftwareSystemNamesAreNotUnique.json",
        Some("A person or software system named \"Name\" already exists."),
    ),
    (
        "RelationshipDescriptionsAreNotUnique.json",
        Some(
            "A relationship with the description \"Uses\" already exists between \"User\" and \"Software System\".",
        ),
    ),
    (
        "RelationshipReferencedByViewIsMissingFromTheModel.json",
        Some(
            "The view with key \"SystemLandscape\" references a relationship (id=4), but that relationship does not exist in the model.",
        ),
    ),
    (
        "SoftwareSystemAssociatedWithContainerViewIsMissingFromTheModel.json",
        Some(
            "The container view with key \"Containers\" is associated with a software system (id=2), but that element does not exist in the model.",
        ),
    ),
    (
        "SoftwareSystemAssociatedWithDeploymentViewIsMissingFromTheModel.json",
        Some(
            "The deployment view with key \"Deployment\" is associated with a software system (id=2), but that element does not exist in the model.",
        ),
    ),
    (
        "SoftwareSystemAssociatedWithSystemContextViewIsMissingFromTheModel.json",
        Some(
            "The system context view with key \"SystemContext\" is associated with a software system (id=2), but that element does not exist in the model.",
        ),
    ),
    (
        "TopLevelDeploymentNodeNamesAreNotUniqueButTheyExistInDifferentEnvironments.json",
        None,
    ),
    (
        "TopLevelDeploymentNodeNamesAreNotUnique.json",
        Some(
            "A top-level deployment node named \"Deployment Node\" already exists for the environment named \"Default\".",
        ),
    ),
    (
        "ViewAssociatedWithFilteredViewIsMissingFromTheWorkspace.json",
        Some(
            "The filtered view with key \"Filtered\" is based upon a view (key=SystemContext), but that view does not exist in the workspace.",
        ),
    ),
    (
        "ViewKeysAreNotUnique.json",
        Some("A view with the key key already exists."),
    ),
];

#[test]
fn validation_fixtures_match_the_reference_rules_tests() {
    let files = json_files(VALIDATION_DIR);
    assert_eq!(
        files.len(),
        VALIDATION_EXPECTATIONS.len(),
        "vendored validation corpus"
    );
    for path in &files {
        let name = path.file_name().unwrap().to_string_lossy();
        let expected = VALIDATION_EXPECTATIONS
            .iter()
            .find(|(file, _)| *file == name.as_ref())
            .unwrap_or_else(|| panic!("{name}: expectation registered"))
            .1;
        match expected {
            Some(message) => match from_slice(&read_file(path)) {
                Err(JsonError::Validation(actual)) => assert_eq!(
                    actual, message,
                    "{name}: first hydrate() message differs from the reference"
                ),
                Err(other) => panic!("{name}: expected Validation, got {other}"),
                Ok(_) => panic!("{name}: must fail hydrate() like the reference"),
            },
            None => {
                from_slice(&read_file(path))
                    .unwrap_or_else(|e| panic!("{name}: the reference loads this OK, got {e}"));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Jackson binding probes (§6)
// ---------------------------------------------------------------------------

/// Minimal valid workspace; `model` and `views` bodies spliced in verbatim
/// (a trailing comma on either is dropped — never emit duplicate keys,
/// `serde_json` would silently collapse them).
fn workspace_json(model: &str, views: &str) -> String {
    workspace_with_systems(model, "", views)
}

/// As [`workspace_json`], with extra `softwareSystems` entries (needed when
/// instances must link to real elements — `hydrate()` checks the links).
fn workspace_with_systems(model: &str, systems: &str, views: &str) -> String {
    let views = views.trim().trim_end_matches(',');
    let views = if views.is_empty() {
        String::new()
    } else {
        format!(" {views} ")
    };
    let systems = systems.trim().trim_end_matches(',');
    let systems = if systems.is_empty() {
        String::new()
    } else {
        format!(" {systems}")
    };
    format!(
        r#"{{
  "id": 0,
  "name": "Probe",
  "model": {{ "deploymentNodes": [{model}], "softwareSystems": [{systems} ], "people": [],
              "customElements": [], "properties": {{}} }},
  "views": {{{views}}}
}}"#
    )
}

fn node(body: &str) -> String {
    let body = body.trim().trim_end_matches(',');
    let tail = if body.is_empty() {
        String::new()
    } else {
        format!(", {body}")
    };
    format!(r#"{{ "id": "1", "name": "N", "tags": "", "properties": {{}}{tail} }}"#)
}

fn read_err(json: &str) -> JsonError {
    from_slice(json.as_bytes()).expect_err("must fail")
}

#[test]
fn instances_follows_the_java_setter_validation() {
    let ok = |spec: &str| {
        let ws =
            from_slice(workspace_json(&node(&format!(r#""instances": "{spec}""#)), "").as_bytes())
                .unwrap_or_else(|e| panic!("instances {spec:?} must load: {e}"));
        assert_eq!(ws.model.deployment_nodes[0].instances, spec);
    };
    ok("1");
    ok("2..5");
    ok("3..*");
    // `\d*..N` — the middle dots are UNESCAPED regex dots: two arbitrary
    // characters, with backtracking, so even "12N" matches (empty `\d*`).
    ok("12N");
    ok("12*");
    ok("123N");

    // Jackson coerces a JSON NUMBER into the String setter (WRITE-CATALOG §6).
    let ws = from_slice(workspace_json(&node(r#""instances": 4"#), "").as_bytes())
        .expect("numeric instances coerced");
    assert_eq!(ws.model.deployment_nodes[0].instances, "4");

    let err = read_err(&workspace_json(&node(r#""instances": "0""#), ""));
    assert!(
        err.to_string()
            .contains("Number of instances must be a positive integer or a range."),
        "{err}"
    );
    assert!(
        read_err(&workspace_json(&node(r#""instances": "-3""#), ""))
            .to_string()
            .contains("positive integer"),
        "negative integer rejected"
    );
    assert!(
        read_err(&workspace_json(&node(r#""instances": "2..1""#), ""))
            .to_string()
            .contains("Range upper bound must be greater than the lower bound."),
        "reversed range rejected"
    );
    assert!(
        read_err(&workspace_json(&node(r#""instances": "x..y""#), ""))
            .to_string()
            .contains("positive integer"),
        "unparsable range side rejected"
    );
    assert!(
        read_err(&workspace_json(&node(r#""instances": "1..2..3""#), ""))
            .to_string()
            .contains("positive integer"),
        "double range rejected"
    );
}

#[test]
fn absent_instances_keeps_the_field_default() {
    // The `@JsonSetter` is not called when the key is absent — the bean's
    // `"1"` initializer survives (and the writer emits `instances: "1"`).
    let ws = from_slice(workspace_json(&node(r#""technology": "t""#), "").as_bytes())
        .expect("node loads");
    assert_eq!(ws.model.deployment_nodes[0].instances, "1");
}

#[test]
fn health_checks_reject_null_name_or_url_like_the_reference() {
    // `HttpHealthCheck.hashCode()` dereferences BOTH fields on Jackson's
    // first HashSet insert — the reference read NPEs when either key is
    // missing/null (oracle witness 2026-11-14). oxidizr rejects as a value.
    let instance = r#"
      "containerInstances": [ {
        "id": "2", "containerId": "9", "instanceId": 0, "tags": "", "properties": {},
        "healthChecks": [ { "url": "http://e.com" } ]
      } ]"#;
    let err = read_err(&workspace_json(&node(instance), ""));
    assert!(
        matches!(err, JsonError::Read(_)) && err.to_string().contains("name must not be null"),
        "{err}"
    );
    let missing_url = instance.replace(r#"{ "url": "http://e.com" }"#, r#"{ "name": "hc" }"#);
    assert!(
        read_err(&workspace_json(&node(&missing_url), ""))
            .to_string()
            .contains("URL must not be null"),
        "missing url rejected"
    );
    let null_name = instance.replace(
        r#"{ "url": "http://e.com" }"#,
        r#"{ "name": null, "url": "http://e.com" }"#,
    );
    assert!(
        read_err(&workspace_json(&node(&null_name), ""))
            .to_string()
            .contains("name must not be null"),
        "explicit null hits the same NPE in the reference"
    );
}

#[test]
fn health_check_defaults_are_the_bean_initializers() {
    // `interval`/`timeout` default to the Java field initializers 0/0 — the
    // 60 s interval is the DSL parser's default, NOT the reader's (probe
    // golden `m3b-defaults.json` proves both are written back as 0).
    let system = r#"{ "id": "8", "name": "S", "tags": "", "properties": {},
        "containers": [ { "id": "9", "name": "C", "tags": "", "properties": {} } ] }"#;
    let instance = r#"
      "containerInstances": [ {
        "id": "2", "containerId": "9", "instanceId": 0, "tags": "", "properties": {},
        "healthChecks": [ { "name": "hc", "url": "http://e.com" } ]
      } ]"#;
    let json = workspace_with_systems(&node(instance), system, "");
    let ws = from_slice(json.as_bytes()).expect("health check loads");
    let check = &ws.model.deployment_nodes[0].container_instances[0].health_checks[0];
    assert_eq!(check.interval, 0);
    assert_eq!(check.timeout, 0);
}

#[test]
fn automatic_layout_defaults_are_the_bean_initializers() {
    // Oracle probe (`m3b-defaults`): `{}` loads as 0/0/0/false with a NULL
    // rank direction (written back without the key); the DSL's
    // LeftRight/100/50/50/true are NOT reader defaults.
    let json = workspace_json(
        &node(r#""technology": "t""#),
        r#""systemLandscapeViews": [ { "key": "L", "order": 1, "automaticLayout": {},
                                        "elements": [], "relationships": [] } ],"#,
    );
    let ws = from_slice(json.as_bytes()).expect("empty automaticLayout loads");
    let layout = ws.views.system_landscape_views[0]
        .content
        .automatic_layout
        .unwrap();
    assert_eq!(layout.rank_separation, 0);
    assert_eq!(layout.node_separation, 0);
    assert_eq!(layout.edge_separation, 0);
    assert!(!layout.vertices);
    assert!(layout.rank_direction.is_none());

    // A PRESENT-but-unknown `rankDirection` hits `setRankDirection(null)`.
    let bogus = json.replace(
        r#""automaticLayout": {}"#,
        r#""automaticLayout": { "rankDirection": "Sideways" }"#,
    );
    let err = read_err(&bogus);
    assert!(
        err.to_string()
            .contains("A rank direction must be specified."),
        "{err}"
    );
}

#[test]
fn style_colors_are_normalized_by_the_setter() {
    let json = workspace_json(
        &node(r#""technology": "t""#),
        r##""configuration": { "styles": { "elements": [ {
              "tag": "Element", "background": "red", "color": "#AABBCC" } ] } },"##,
    );
    let ws = from_slice(json.as_bytes()).expect("styles load");
    let style = &ws.views.configuration.styles.element_styles[0];
    assert_eq!(style.background.as_deref(), Some("#ff0000"));
    assert_eq!(style.color.as_deref(), Some("#aabbcc"));

    let bogus = json.replace(r##""color": "#AABBCC""##, r#""color": "nope""#);
    assert!(
        read_err(&bogus)
            .to_string()
            .contains("nope is not a valid hex colour code or HTML colour name."),
    );
}

#[test]
fn enterprise_context_views_replace_landscape_views_in_document_order() {
    // Both keys bind the SAME landscape view set (`@JsonSetter` replaces), so
    // the LAST key in DOCUMENT order wins — Jackson binds properties as the
    // stream encounters them, and the reader walks keys in document order.
    let enterprise_last = workspace_json(
        &node(r#""technology": "t""#),
        r#""systemLandscapeViews": [ { "key": "S", "order": 1, "elements": [], "relationships": [] } ],
           "enterpriseContextViews": [ { "key": "E", "order": 2, "elements": [], "relationships": [] } ],"#,
    );
    let ws = from_slice(enterprise_last.as_bytes()).expect("loads");
    assert_eq!(ws.views.system_landscape_views.len(), 1);
    assert_eq!(ws.views.system_landscape_views[0].base.key, "E");

    let landscape_last = workspace_json(
        &node(r#""technology": "t""#),
        r#""enterpriseContextViews": [ { "key": "E", "order": 2, "elements": [], "relationships": [] } ],
           "systemLandscapeViews": [ { "key": "S", "order": 1, "elements": [], "relationships": [] } ],"#,
    );
    let ws = from_slice(landscape_last.as_bytes()).expect("loads");
    assert_eq!(ws.views.system_landscape_views[0].base.key, "S");
}

#[test]
fn themes_alias_dedupe_and_trim() {
    // `theme` (singular) aliases into `themes[]` (`@JsonSetter`); URL themes
    // dedupe (`addTheme`), whitespace is trimmed, empties skipped. Which
    // bare names count as "installed" is filesystem-dependent in the
    // reference, so oxidizr keeps every name it sees (D3 tolerance).
    let json = workspace_json(
        &node(r#""technology": "t""#),
        r#""configuration": { "themes": [ "http://e.com/t.json", "  http://e.com/t.json  ",
              "", "http://e.com/u.json" ], "theme": "http://e.com/t.json", "properties": {} },"#,
    );
    let ws = from_slice(json.as_bytes()).expect("loads");
    assert_eq!(
        ws.views.configuration.themes,
        vec![
            "http://e.com/t.json".to_string(),
            "http://e.com/u.json".to_string(),
        ]
    );
}

#[test]
fn configuration_users_keep_a_null_role_as_absent() {
    // The probe golden `m3b-defaults.json`: `role: "Bogus"` binds null and
    // the writer omits the key again (`users` DO round-trip — `setUsers` is
    // package-private, Jackson setter visibility ANY).
    let mut ws = Workspace::default();
    ws.configuration.users = vec![User {
        username: "bob".to_string(),
        role: None,
    }];
    let bytes = to_vec(&ws).unwrap();
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.contains(r#""users" : [ {"#), "{text}");
    assert!(text.contains(r#""username" : "bob""#), "{text}");
    assert!(!text.contains("role"), "null role must be omitted: {text}");
    let back = from_slice(text.as_bytes()).unwrap();
    assert_eq!(back, ws);
}

#[test]
fn last_modified_date_binds_iso_text_and_millis() {
    let base = workspace_json(&node(r#""technology": "t""#), "");
    let iso = base.replace(
        r#""name": "Probe""#,
        r#""name": "Probe", "lastModifiedDate": "2026-11-14T08:00:00Z""#,
    );
    let ws = from_slice(iso.as_bytes()).expect("loads");
    assert_eq!(ws.last_modified_date, Some(1_794_643_200)); // 2026-11-14T08:00:00Z

    let millis = base.replace(
        r#""name": "Probe""#,
        r#""name": "Probe", "lastModifiedDate": 1794643200000"#,
    );
    let ws = from_slice(millis.as_bytes()).expect("loads");
    assert_eq!(ws.last_modified_date, Some(1_794_643_200));

    // The epoch millis are written back in the reference's date format and
    // re-read identically (byte-stable through `to_vec`).
    let bytes = to_vec(&ws).unwrap();
    let text = String::from_utf8(bytes.clone()).unwrap();
    assert!(
        text.contains(r#""lastModifiedDate" : "2026-11-14T08:00:00Z""#),
        "{text}"
    );
    let back: Workspace = from_slice(&bytes).unwrap();
    assert_eq!(back.last_modified_date, ws.last_modified_date);
}

// ---------------------------------------------------------------------------
// Property maps (V-11 corollary)
// ---------------------------------------------------------------------------

mod strategy {
    use proptest::prelude::*;

    /// JSON-safe printable key/value strings (quotes/backslashes would only
    /// test the escaper, which the goldens already pin).
    pub fn strings() -> impl Strategy<Value = String> {
        proptest::string::string_regex(r#"[^\x00"\\]{0,12}"#).expect("valid regex")
    }
}

use proptest::prop_assert_eq;

proptest::proptest! {
    /// The Java `HashMap` bucket-order replay is idempotent: writing a
    /// property map, re-reading it in (bucket) document order, and writing
    /// again produce identical bytes — `H(H(I)) == H(I)` for every insertion
    /// sequence `I`. This is what makes the reference's own JSON output
    /// byte-stable under JSON→JSON export, and hence the goldens stable.
    #[test]
    fn property_maps_round_trip_byte_stably(
        entries in proptest::collection::vec(
            (strategy::strings(), strategy::strings()),
            0..8,
        ),
    ) {
        let mut ws = Workspace::default();
        for (key, value) in entries {
            ws.properties.insert(key, value);
        }
        // Panicking (not rejecting) is intended: a default workspace with a
        // property map must write and re-read — a failure is a bug, and
        // proptest shrinks the panicking case.
        let first = to_vec(&ws).unwrap();
        let back: Workspace = from_slice(&first).unwrap();
        let second = to_vec(&back).unwrap();
        prop_assert_eq!(&first, &second);
    }
}
