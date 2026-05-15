use rf_core::domain::{
    BlockerConcern, CommentRecord, CommentSource, CommentType, GateConfigSnapshot,
    PullRequestSummary, ScanArtifact, Status,
};
use rf_core::{build_review_threads, gate_scan};
use serde::Deserialize;

const NOISE_FIXTURES: &str =
    include_str!("../../../fixtures/reviews/fixtures-reviews-noise-ja-100.yaml");
const TRUE_BLOCKER_FIXTURES: &str =
    include_str!("../../../fixtures/reviews/fixtures-reviews-true-blockers-ja-50.yaml");
const TRUE_BLOCKER_NATURAL_FIXTURES: &str =
    include_str!("../../../fixtures/reviews/true_blockers_natural_ja.yaml");
const CONCERN_FALSE_POSITIVE_FIXTURES: &str =
    include_str!("../../../fixtures/reviews/concern_false_positives_ja.yaml");
const FAILURE_MODE_FALSE_POSITIVE_FIXTURES: &str =
    include_str!("../../../fixtures/reviews/failure_mode_false_positives_ja.yaml");
const NOISE_EXACT_FIXTURES: &str = include_str!("../../../fixtures/reviews/noise_exact_ja.yaml");

#[derive(Debug, Deserialize)]
struct ReviewFixtureFile {
    version: u64,
    kind: String,
    language: String,
    #[serde(default)]
    notes: Vec<String>,
    cases: Vec<ReviewFixtureCase>,
}

#[derive(Debug, Deserialize)]
struct ReviewFixtureCase {
    id: String,
    category: String,
    #[serde(default)]
    looks_like_concern_ja: String,
    #[serde(default)]
    trigger_token_ja: String,
    comment_ja: String,
    expected: ExpectedFixture,
    #[serde(default)]
    failure_mode_ja: String,
    #[serde(default)]
    evidence_ja: String,
    #[serde(default)]
    impact_on_pr_ja: String,
    #[serde(default)]
    alternative_ja: String,
}

#[derive(Debug, Deserialize)]
struct ExpectedFixture {
    #[serde(rename = "type")]
    comment_type: CommentType,
    blocker: bool,
    concern: Option<BlockerConcern>,
}

#[test]
fn review_fixtures_noise_cases_remain_non_blocking() {
    let fixtures = load_fixture_file(NOISE_FIXTURES);
    assert_eq!(fixtures.version, 1, "unexpected fixture version");
    assert_eq!(fixtures.language, "ja", "unexpected fixture language");
    assert!(
        fixtures.kind.contains("noise"),
        "unexpected fixture kind: {}",
        fixtures.kind
    );
    assert!(
        !fixtures.notes.is_empty(),
        "noise fixture file should carry notes"
    );

    let mut failures = Vec::new();
    for case in &fixtures.cases {
        let body = case.comment_ja.clone();
        let gate = gate_scan(
            &synthetic_scan(&case.id, &body),
            &GateConfigSnapshot::default(),
            &[],
        );
        let actual = gate
            .classified_comments
            .first()
            .expect("classified fixture comment");

        assert!(
            !case.expected.blocker,
            "noise fixture {} is malformed: expected.blocker must be false",
            case.id
        );
        if !gate.residual_blockers.is_empty() {
            failures.push(format!(
                "{} ({}) became blocking\nexpected_type={:?} actual_type={:?}\ncomment={}",
                case.id,
                case.category,
                case.expected.comment_type,
                actual.comment_type,
                case.comment_ja
            ));
        }
        if !matches_noise_type(case.expected.comment_type, actual.comment_type) {
            failures.push(format!(
                "{} ({}) changed non-blocking classification\nexpected_type={:?} actual_type={:?}\ncomment={}",
                case.id, case.category, case.expected.comment_type, actual.comment_type, case.comment_ja
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "noise fixtures failed:\n{}",
        failures.join("\n\n")
    );
}

#[test]
fn review_fixtures_true_blockers_remain_blocking_with_expected_concern() {
    let fixtures = load_fixture_file(TRUE_BLOCKER_FIXTURES);
    assert_eq!(fixtures.version, 1, "unexpected fixture version");
    assert_eq!(fixtures.language, "ja", "unexpected fixture language");
    assert!(
        fixtures.kind.contains("blocker"),
        "unexpected fixture kind: {}",
        fixtures.kind
    );
    assert!(
        !fixtures.notes.is_empty(),
        "true blocker fixture file should carry notes"
    );

    let mut failures = Vec::new();
    for case in &fixtures.cases {
        assert!(
            case.expected.blocker,
            "true blocker fixture {} is malformed: expected.blocker must be true",
            case.id
        );
        assert!(
            !case.failure_mode_ja.trim().is_empty(),
            "fixture {} is missing failure_mode_ja",
            case.id
        );
        assert!(
            !case.evidence_ja.trim().is_empty(),
            "fixture {} is missing evidence_ja",
            case.id
        );
        assert!(
            !case.impact_on_pr_ja.trim().is_empty(),
            "fixture {} is missing impact_on_pr_ja",
            case.id
        );

        let body = blocker_body(case);
        let gate = gate_scan(
            &synthetic_scan(&case.id, &body),
            &GateConfigSnapshot::default(),
            &[],
        );
        let actual = gate
            .classified_comments
            .first()
            .expect("classified fixture comment");
        let blocker = gate.residual_blockers.first();

        if blocker.is_none() {
            failures.push(format!(
                "{} ({}) did not stay blocking\nexpected_concern={:?} actual_type={:?} actual_concern={:?} actual_failure_mode={:?} actual_evidence_class={:?} present_pr_impact={}\ncomment={}\nbody={}",
                case.id,
                case.category,
                case.expected.concern,
                actual.comment_type,
                actual.concern,
                actual.failure_mode,
                actual.evidence_class,
                actual.present_pr_impact,
                case.comment_ja,
                body
            ));
            continue;
        }
        if actual.comment_type != CommentType::Blocker {
            failures.push(format!(
                "{} ({}) lost blocker type\nexpected_concern={:?} actual_concern={:?}\ncomment={}",
                case.id, case.category, case.expected.concern, actual.concern, case.comment_ja
            ));
        }
        if blocker.map(|value| value.concern) != case.expected.concern {
            failures.push(format!(
                "{} ({}) changed concern\nexpected_concern={:?} actual_concern={:?}\ncomment={}",
                case.id,
                case.category,
                case.expected.concern,
                blocker.map(|value| value.concern),
                case.comment_ja
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "true blocker fixtures failed:\n{}",
        failures.join("\n\n")
    );
}

#[test]
fn review_fixtures_true_blockers_natural_remain_blocking_with_expected_concern() {
    let fixtures = load_fixture_file(TRUE_BLOCKER_NATURAL_FIXTURES);
    assert_eq!(fixtures.version, 1, "unexpected fixture version");
    assert_eq!(fixtures.language, "ja", "unexpected fixture language");
    assert!(
        fixtures.kind.contains("natural"),
        "unexpected fixture kind: {}",
        fixtures.kind
    );
    assert!(
        !fixtures.notes.is_empty(),
        "true blocker natural fixture file should carry notes"
    );

    let mut failures = Vec::new();
    for case in &fixtures.cases {
        assert!(
            case.expected.blocker,
            "natural blocker fixture {} is malformed: expected.blocker must be true",
            case.id
        );
        assert!(
            !case.failure_mode_ja.trim().is_empty(),
            "fixture {} is missing failure_mode_ja",
            case.id
        );
        assert!(
            !case.evidence_ja.trim().is_empty(),
            "fixture {} is missing evidence_ja",
            case.id
        );
        assert!(
            !case.impact_on_pr_ja.trim().is_empty(),
            "fixture {} is missing impact_on_pr_ja",
            case.id
        );

        let gate = gate_scan(
            &synthetic_scan(&case.id, &case.comment_ja),
            &GateConfigSnapshot::default(),
            &[],
        );
        let actual = gate
            .classified_comments
            .first()
            .expect("classified fixture comment");
        let blocker = gate.residual_blockers.first();

        if blocker.is_none() {
            failures.push(format!(
                "{} ({}) did not stay blocking\nexpected_concern={:?} actual_type={:?} actual_concern={:?}\ncomment={}",
                case.id,
                case.category,
                case.expected.concern,
                actual.comment_type,
                actual.concern,
                case.comment_ja
            ));
            continue;
        }
        if actual.comment_type != CommentType::Blocker {
            failures.push(format!(
                "{} ({}) lost blocker type\nexpected_concern={:?} actual_concern={:?}\ncomment={}",
                case.id, case.category, case.expected.concern, actual.concern, case.comment_ja
            ));
        }
        if blocker.map(|value| value.concern) != case.expected.concern {
            failures.push(format!(
                "{} ({}) changed concern\nexpected_concern={:?} actual_concern={:?}\ncomment={}",
                case.id,
                case.category,
                case.expected.concern,
                blocker.map(|value| value.concern),
                case.comment_ja
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "true blocker natural fixtures failed:\n{}",
        failures.join("\n\n")
    );
}

#[test]
fn review_fixtures_concern_false_positive_cases_do_not_extract_concern() {
    let fixtures = load_fixture_file(CONCERN_FALSE_POSITIVE_FIXTURES);
    assert_eq!(fixtures.version, 1, "unexpected fixture version");
    assert_eq!(fixtures.language, "ja", "unexpected fixture language");
    assert!(
        fixtures.kind.contains("false_positive"),
        "unexpected fixture kind: {}",
        fixtures.kind
    );
    assert!(
        !fixtures.notes.is_empty(),
        "concern false-positive fixture file should carry notes"
    );

    let mut failures = Vec::new();
    for case in &fixtures.cases {
        assert!(
            !case.expected.blocker,
            "false-positive fixture {} is malformed: expected.blocker must be false",
            case.id
        );

        let gate = gate_scan(
            &synthetic_scan(&case.id, &case.comment_ja),
            &GateConfigSnapshot::default(),
            &[],
        );
        let actual = gate
            .classified_comments
            .first()
            .expect("classified fixture comment");

        if !gate.residual_blockers.is_empty() || actual.comment_type == CommentType::Blocker {
            failures.push(format!(
                "{} ({}) became blocking\ntrigger_token={} looks_like_concern={}\nactual_type={:?} actual_concern={:?}\ncomment={}",
                case.id,
                case.category,
                case.trigger_token_ja,
                case.looks_like_concern_ja,
                actual.comment_type,
                actual.concern,
                case.comment_ja
            ));
        }

        if actual.concern.is_some() {
            failures.push(format!(
                "{} ({}) extracted a false concern\ntrigger_token={} looks_like_concern={}\nactual_type={:?} actual_concern={:?}\ncomment={}",
                case.id,
                case.category,
                case.trigger_token_ja,
                case.looks_like_concern_ja,
                actual.comment_type,
                actual.concern,
                case.comment_ja
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "concern false-positive fixtures failed:\n{}",
        failures.join("\n\n")
    );
}

#[test]
fn review_fixtures_failure_mode_false_positive_cases_do_not_extract_failure_mode() {
    let fixtures = load_fixture_file(FAILURE_MODE_FALSE_POSITIVE_FIXTURES);
    assert_eq!(fixtures.version, 1, "unexpected fixture version");
    assert_eq!(fixtures.language, "ja", "unexpected fixture language");
    assert!(
        fixtures.kind.contains("failure_mode_false_positive"),
        "unexpected fixture kind: {}",
        fixtures.kind
    );
    assert!(
        !fixtures.notes.is_empty(),
        "failure-mode false-positive fixture file should carry notes"
    );

    let mut failures = Vec::new();
    for case in &fixtures.cases {
        assert!(
            !case.expected.blocker,
            "failure-mode false-positive fixture {} is malformed: expected.blocker must be false",
            case.id
        );

        let gate = gate_scan(
            &synthetic_scan(&case.id, &case.comment_ja),
            &GateConfigSnapshot::default(),
            &[],
        );
        let actual = gate
            .classified_comments
            .first()
            .expect("classified fixture comment");

        if !gate.residual_blockers.is_empty() || actual.comment_type == CommentType::Blocker {
            failures.push(format!(
                "{} ({}) became blocking\nexpected_type={:?} actual_type={:?} actual_failure_mode={:?}\ncomment={}",
                case.id,
                case.category,
                case.expected.comment_type,
                actual.comment_type,
                actual.failure_mode,
                case.comment_ja
            ));
        }
        if actual.comment_type != case.expected.comment_type {
            failures.push(format!(
                "{} ({}) changed non-blocking classification\nexpected_type={:?} actual_type={:?} actual_failure_mode={:?}\ncomment={}",
                case.id,
                case.category,
                case.expected.comment_type,
                actual.comment_type,
                actual.failure_mode,
                case.comment_ja
            ));
        }
        if actual.failure_mode.is_some() {
            failures.push(format!(
                "{} ({}) extracted a metalinguistic failure mode\nactual_type={:?} actual_failure_mode={:?}\ncomment={}",
                case.id,
                case.category,
                actual.comment_type,
                actual.failure_mode,
                case.comment_ja
            ));
        }
        if actual.concern.is_some() {
            failures.push(format!(
                "{} ({}) extracted unexpected concern\nactual_type={:?} actual_concern={:?} actual_failure_mode={:?}\ncomment={}",
                case.id,
                case.category,
                actual.comment_type,
                actual.concern,
                actual.failure_mode,
                case.comment_ja
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "failure-mode false-positive fixtures failed:\n{}",
        failures.join("\n\n")
    );
}

#[test]
fn review_fixtures_exact_non_blocking_cases_keep_exact_types() {
    let fixtures = load_fixture_file(NOISE_EXACT_FIXTURES);
    assert_eq!(fixtures.version, 1, "unexpected fixture version");
    assert_eq!(fixtures.language, "ja", "unexpected fixture language");
    assert!(
        fixtures.kind.contains("exact_non_blocking"),
        "unexpected fixture kind: {}",
        fixtures.kind
    );
    assert!(
        !fixtures.notes.is_empty(),
        "exact non-blocking fixture file should carry notes"
    );

    let mut failures = Vec::new();
    for case in &fixtures.cases {
        assert!(
            !case.expected.blocker,
            "exact fixture {} is malformed: expected.blocker must be false",
            case.id
        );
        assert!(
            case.expected.concern.is_none(),
            "exact fixture {} is malformed: expected.concern must be null",
            case.id
        );

        let gate = gate_scan(
            &synthetic_scan(&case.id, &case.comment_ja),
            &GateConfigSnapshot::default(),
            &[],
        );
        let actual = gate
            .classified_comments
            .first()
            .expect("classified fixture comment");

        if !gate.residual_blockers.is_empty() || actual.comment_type == CommentType::Blocker {
            failures.push(format!(
                "{} ({}) became blocking\nexpected_type={:?} actual_type={:?} actual_concern={:?}\ncomment={}",
                case.id,
                case.category,
                case.expected.comment_type,
                actual.comment_type,
                actual.concern,
                case.comment_ja
            ));
        }
        if actual.comment_type != case.expected.comment_type {
            failures.push(format!(
                "{} ({}) changed exact non-blocking classification\nexpected_type={:?} actual_type={:?} actual_concern={:?}\ncomment={}",
                case.id,
                case.category,
                case.expected.comment_type,
                actual.comment_type,
                actual.concern,
                case.comment_ja
            ));
        }
        if actual.concern.is_some() {
            failures.push(format!(
                "{} ({}) extracted unexpected concern\nexpected_type={:?} actual_type={:?} actual_concern={:?}\ncomment={}",
                case.id,
                case.category,
                case.expected.comment_type,
                actual.comment_type,
                actual.concern,
                case.comment_ja
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "exact non-blocking fixtures failed:\n{}",
        failures.join("\n\n")
    );
}

fn load_fixture_file(contents: &str) -> ReviewFixtureFile {
    serde_yaml::from_str(contents).expect("load review fixtures")
}

fn synthetic_scan(comment_id: &str, body: &str) -> ScanArtifact {
    let comment = CommentRecord {
        comment_id: comment_id.to_owned(),
        thread_id: comment_id.to_owned(),
        author: String::from("reviewer-fixture"),
        body: body.to_owned(),
        path: Some(String::from("src/fixture.rs")),
        source: CommentSource::ReviewComment,
        reply_to_comment_id: None,
        created_at: Some(String::from("2026-03-31T00:00:00Z")),
        line: Some(10),
        original_line: Some(10),
    };

    ScanArtifact {
        status: Status::Ok,
        data_coverage: rf_core::domain::DataCoverage::Full,
        review_signal: rf_core::domain::ReviewSignal::Unknown,
        reason: None,
        scan_partial: false,
        repo_root: Some(String::from("/tmp/review-firewall")),
        branch: Some(String::from("fixture/tests")),
        pr: PullRequestSummary {
            number: Some(1),
            title: String::from("Fixture review"),
            ..PullRequestSummary::default()
        },
        files_changed: 1,
        review_comments: 1,
        threads: 1,
        codeowners_found: false,
        policy_found: false,
        product_boundary: Default::default(),
        changed_files: vec![String::from("src/fixture.rs")],
        comments: vec![comment.clone()],
        issue_comments: Vec::new(),
        review_threads: build_review_threads(&[comment]),
        partial_sources: Vec::new(),
        warnings: Vec::new(),
    }
}

fn blocker_body(case: &ReviewFixtureCase) -> String {
    let mut lines = vec![
        case.comment_ja.clone(),
        format!("mode: {}", case.failure_mode_ja),
        format!("because {}", case.evidence_ja),
        format!("in this pr: {}", case.impact_on_pr_ja),
    ];
    if !case.alternative_ja.trim().is_empty() {
        lines.push(format!("alternative: {}", case.alternative_ja));
    }
    lines.join("\n")
}

fn matches_noise_type(expected: CommentType, actual: CommentType) -> bool {
    expected == actual || (is_non_blocking_band(expected) && is_non_blocking_band(actual))
}

fn is_non_blocking_band(comment_type: CommentType) -> bool {
    matches!(
        comment_type,
        CommentType::Question | CommentType::Suggestion | CommentType::Nit
    )
}
