use std::collections::BTreeSet;

use jsonschema::{meta, validator_for};
use rf_core::domain::{
    DataCoverage, DraftReplyArtifact, GateArtifact, LatestPointer, ReviewSignal, ScanArtifact,
    Status,
};
use serde::Deserialize;
use serde_json::Value;

const DEMO_LATEST: &str =
    include_str!("../../../docs/demos/anonymized-dogfood-run/run/latest.json");
const DEMO_SCAN: &str = include_str!(
    "../../../docs/demos/anonymized-dogfood-run/run/20260509T120800.671751000Z/scan.json"
);
const DEMO_GATE: &str = include_str!(
    "../../../docs/demos/anonymized-dogfood-run/run/20260509T120800.671751000Z/gate.json"
);
const DEMO_DRAFT_REPLY: &str = include_str!(
    "../../../docs/demos/anonymized-dogfood-run/run/20260509T120800.671751000Z/draft_reply.json"
);
const DEMO_REPORT: &str = include_str!(
    "../../../docs/demos/anonymized-dogfood-run/run/20260509T120800.671751000Z/report.md"
);
const DEMO_ESCALATION: &str = include_str!(
    "../../../docs/demos/anonymized-dogfood-run/run/20260509T120800.671751000Z/escalation.md"
);
const MANUAL_LABEL_CORPUS: &str =
    include_str!("../../../fixtures/validation/manual_labels_v0.1.yaml");
const README_EN: &str = include_str!("../../../README.md");
const README_JA: &str = include_str!("../../../README_JA.md");
const VALIDATION_DOC: &str = include_str!("../../../docs/VALIDATION.md");
const DEFERRED_DOC: &str = include_str!("../../../DEFERRED.md");
const DEMO_README: &str = include_str!("../../../docs/demos/anonymized-dogfood-run/README.md");
const FREEZE_AUDIT_DOC: &str = include_str!("../../../docs/v0.1-freeze-audit.md");
const SCAN_SCHEMA: &str = include_str!("../../../schemas/scan.schema.json");
const GATE_SCHEMA: &str = include_str!("../../../schemas/gate.schema.json");

#[derive(Debug, Deserialize)]
struct ManualLabelCorpus {
    version: u64,
    kind: String,
    language: String,
    sample_target: usize,
    human_confirmed: bool,
    release_usable: bool,
    release_usable_reason: String,
    recorded_metrics: RecordedMetrics,
    #[serde(default)]
    notes: Vec<String>,
    source_run: SourceRun,
    cases: Vec<ManualLabelCase>,
}

#[derive(Debug, Default, Deserialize)]
struct RecordedMetrics {
    false_residual_rate: Option<f64>,
    missed_obvious_blocker_rate: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct SourceRun {
    demo_path: String,
    comments_analyzed: usize,
    candidate_blockers: usize,
    residual_blockers: usize,
}

#[derive(Debug, Deserialize)]
struct ManualLabelCase {
    comment_id: String,
    source_type: SourceType,
    path: String,
    observed_bucket: ObservedBucket,
    manual_label: ManualLabel,
    obvious_blocker: bool,
    reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SourceType {
    Human,
    AiBot,
    Ci,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ObservedBucket {
    ResidualBlocker,
    CandidateOnly,
    NonBlocker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ManualLabel {
    TrueBlocker,
    FalseBlocker,
    Question,
    Suggestion,
    Nit,
    DesignDebate,
    Unknown,
    Noise,
}

#[test]
fn checked_in_demo_artifacts_follow_current_contract() {
    let latest: LatestPointer = serde_json::from_str(DEMO_LATEST).expect("demo latest pointer");
    let scan: ScanArtifact = serde_json::from_str(DEMO_SCAN).expect("demo scan artifact");
    let gate: GateArtifact = serde_json::from_str(DEMO_GATE).expect("demo gate artifact");
    let draft: DraftReplyArtifact =
        serde_json::from_str(DEMO_DRAFT_REPLY).expect("demo draft reply artifact");

    assert_eq!(latest.timestamp, "20260509T120800.671751000Z");

    assert_eq!(scan.status, Status::Ok);
    assert_eq!(scan.data_coverage, DataCoverage::Full);
    assert_eq!(scan.review_signal, ReviewSignal::Unknown);
    assert_eq!(scan.files_changed, 76);
    assert_eq!(scan.review_comments, 274);
    assert_eq!(scan.threads, 296);
    assert!(scan.codeowners_found);
    assert!(scan.policy_found);

    assert_eq!(gate.status, Status::Ok);
    assert_eq!(gate.data_coverage, DataCoverage::Full);
    assert_eq!(gate.review_signal, ReviewSignal::Blocked);
    assert_eq!(gate.comments_analyzed, 397);
    assert_eq!(gate.residual_blockers.len(), 11);
    assert_eq!(gate.candidate_blockers.len(), 45);
    assert_eq!(gate.counts.questions, 154);
    assert_eq!(gate.counts.suggestions, 135);
    assert_eq!(gate.counts.nits, 97);
    let review_decision_summary = gate
        .review_decision_summary
        .as_ref()
        .expect("demo review decision summary");
    assert_eq!(
        review_decision_summary.states,
        vec![String::from("COMMENTED")]
    );
    assert!(!review_decision_summary.changes_requested);
    assert!(review_decision_summary.informational_only);

    assert_eq!(draft.status, Status::Ok);
    assert_eq!(draft.data_coverage, DataCoverage::Full);
    assert_eq!(draft.review_signal, ReviewSignal::Blocked);
    assert_eq!(draft.target_comment_id.as_deref(), Some("demo-r001"));

    assert!(DEMO_REPORT.contains("RUN_STATUS: OK"));
    assert!(DEMO_REPORT.contains("DATA_COVERAGE: FULL"));
    assert!(DEMO_REPORT.contains("REVIEW_SIGNAL: BLOCKED"));
    assert!(DEMO_REPORT.contains("RESIDUAL_BLOCKERS: 11"));
    assert!(DEMO_REPORT.contains("REVIEW_DECISIONS: COMMENTED (informational only)"));
    assert!(!DEMO_REPORT.contains("style=flat"));
    assert!(DEMO_ESCALATION.contains("No ADR/RFC candidates were found."));
}

#[test]
fn manual_label_corpus_scaffold_is_well_formed() {
    let gate: GateArtifact = serde_json::from_str(DEMO_GATE).expect("demo gate artifact");
    let corpus: ManualLabelCorpus =
        serde_yaml::from_str(MANUAL_LABEL_CORPUS).expect("manual label corpus");

    assert_eq!(corpus.version, 1);
    assert!(corpus.kind.contains("manual_label"));
    assert_eq!(corpus.language, "en");
    assert_eq!(corpus.sample_target, 50);
    assert!(!corpus.notes.is_empty());
    assert!(
        !corpus.release_usable_reason.trim().is_empty(),
        "release_usable_reason should explain why the corpus is or is not ready"
    );
    if !corpus.human_confirmed {
        assert!(
            corpus.release_usable_reason.contains("human-confirmed"),
            "non-human-confirmed corpus should say that release validation still needs human confirmation"
        );
    }
    if corpus.cases.len() < corpus.sample_target {
        assert!(
            corpus.release_usable_reason.contains("50")
                || corpus.release_usable_reason.contains("sample target"),
            "release_usable_reason should explain when the sample target is still unmet"
        );
    }
    if corpus.recorded_metrics.false_residual_rate.is_none() {
        assert!(
            corpus.release_usable_reason.contains("false_residual_rate"),
            "release_usable_reason should call out a missing false_residual_rate"
        );
    }
    if corpus
        .recorded_metrics
        .missed_obvious_blocker_rate
        .is_none()
    {
        assert!(
            corpus
                .release_usable_reason
                .contains("missed_obvious_blocker_rate"),
            "release_usable_reason should call out a missing missed_obvious_blocker_rate"
        );
    }

    assert_eq!(
        corpus.source_run.demo_path,
        "docs/demos/anonymized-dogfood-run/run/20260509T120800.671751000Z"
    );
    assert_eq!(corpus.source_run.comments_analyzed, 397);
    assert_eq!(corpus.source_run.candidate_blockers, 45);
    assert_eq!(corpus.source_run.residual_blockers, 11);

    assert!(
        !corpus.cases.is_empty(),
        "manual label corpus should not be empty"
    );

    let mut ids = BTreeSet::new();
    let demo_ids = demo_traceable_ids(&gate);
    let mut saw_residual = false;
    let mut saw_obvious = false;
    let mut saw_false_blocker = false;
    let mut saw_human = false;
    let mut saw_ai_bot = false;
    let mut saw_ci = false;
    let mut saw_unknown = false;

    for case in &corpus.cases {
        assert!(
            ids.insert(case.comment_id.clone()),
            "duplicate comment id in corpus: {}",
            case.comment_id
        );
        assert!(
            demo_ids.contains(case.comment_id.as_str()),
            "corpus id is not traceable from checked-in demo artifacts: {}",
            case.comment_id
        );
        assert!(
            !case.path.trim().is_empty(),
            "path should not be empty for {}",
            case.comment_id
        );
        assert!(
            !case.reason.trim().is_empty(),
            "reason should not be empty for {}",
            case.comment_id
        );

        saw_residual |= case.observed_bucket == ObservedBucket::ResidualBlocker;
        saw_obvious |= case.obvious_blocker;
        saw_false_blocker |= case.manual_label == ManualLabel::FalseBlocker;
        saw_human |= case.source_type == SourceType::Human;
        saw_ai_bot |= case.source_type == SourceType::AiBot;
        saw_ci |= case.source_type == SourceType::Ci;
        saw_unknown |= case.source_type == SourceType::Unknown;
    }

    assert!(
        saw_residual,
        "corpus should include at least one residual blocker row"
    );
    assert!(
        saw_obvious,
        "corpus should include at least one obvious blocker row"
    );
    assert!(
        saw_false_blocker,
        "corpus should include at least one false_blocker manual label"
    );
    assert!(saw_human, "corpus should include a human review source");
    assert!(saw_ai_bot, "corpus should include an ai_bot review source");
    assert!(saw_ci, "corpus should include a ci review source");
    assert!(
        saw_unknown,
        "corpus should include an unknown review source"
    );
}

#[test]
fn checked_in_manual_label_corpus_matches_current_scaffold_state() {
    let corpus: ManualLabelCorpus =
        serde_yaml::from_str(MANUAL_LABEL_CORPUS).expect("manual label corpus");

    assert_eq!(corpus.cases.len(), 12);
    assert!(!corpus.human_confirmed);
    assert!(!corpus.release_usable);
    assert_eq!(corpus.sample_target, 50);
    assert!(
        corpus.recorded_metrics.false_residual_rate.is_none(),
        "current checked-in scaffold should not claim a recorded false_residual_rate yet"
    );
    assert!(
        corpus
            .recorded_metrics
            .missed_obvious_blocker_rate
            .is_none(),
        "current checked-in scaffold should not claim a recorded missed_obvious_blocker_rate yet"
    );
}

#[test]
fn release_usable_corpus_requires_sample_size_metrics_and_human_confirmation() {
    let corpus: ManualLabelCorpus =
        serde_yaml::from_str(MANUAL_LABEL_CORPUS).expect("manual label corpus");

    assert!(
        !corpus.release_usable || corpus.human_confirmed,
        "release_usable corpus must be human-confirmed"
    );

    if corpus.release_usable {
        assert!(
            corpus.cases.len() >= corpus.sample_target,
            "release_usable corpus must meet or exceed the sample target"
        );
        let recorded_false_residual = corpus
            .recorded_metrics
            .false_residual_rate
            .expect("release_usable corpus should record false_residual_rate");
        let recorded_missed_obvious = corpus
            .recorded_metrics
            .missed_obvious_blocker_rate
            .expect("release_usable corpus should record missed_obvious_blocker_rate");

        assert_rate_matches(
            recorded_false_residual,
            derive_false_residual_rate(&corpus)
                .expect("release_usable corpus should have residual rows to score"),
            "false_residual_rate",
        );
        assert_rate_matches(
            recorded_missed_obvious,
            derive_missed_obvious_blocker_rate(&corpus)
                .expect("release_usable corpus should have obvious blockers to score"),
            "missed_obvious_blocker_rate",
        );
    }
}

#[test]
fn public_demo_numbers_are_synchronized() {
    for document in [README_EN, README_JA, DEMO_README] {
        assert!(document.contains("397 comments analyzed"));
        assert!(document.contains("45 candidate blockers"));
        assert!(document.contains("11 residual blockers"));
    }

    assert!(VALIDATION_DOC.contains("comments_analyzed: 397"));
    assert!(VALIDATION_DOC.contains("candidate_blockers: 45"));
    assert!(VALIDATION_DOC.contains("residual_blockers: 11"));
    assert!(VALIDATION_DOC.contains("12-row seed scaffold"));
    assert!(VALIDATION_DOC.contains("sampled_rows: 12"));
    assert!(VALIDATION_DOC.contains("target_release_sample: 50"));
    assert!(VALIDATION_DOC.contains("human_confirmed: false"));
    assert!(VALIDATION_DOC.contains("release_usable: false"));
    assert!(VALIDATION_DOC.contains("release_usable_reason"));
    assert!(VALIDATION_DOC.contains("false_residual_rate: not yet recorded"));
    assert!(VALIDATION_DOC.contains("missed_obvious_blocker_rate: not yet recorded"));
    assert!(VALIDATION_DOC.contains("reaching 50 rows is necessary but not sufficient"));
    assert!(VALIDATION_DOC.contains("the checked-in sample must be human-confirmed"));
    assert!(VALIDATION_DOC.contains("shortest round-trip IEEE-754 `f64` decimal representation"));
    assert!(DEMO_README.contains("12-row seed sample"));
    assert!(DEFERRED_DOC.contains("12-row traceable scaffold"));
    assert!(DEFERRED_DOC.contains("human-confirmed 50-comment sample"));
    assert!(
        !FREEZE_AUDIT_DOC
            .contains("First-class downstream `reviewDecision` behavior is explicitly deferred.")
    );
    assert!(
        FREEZE_AUDIT_DOC
            .contains("Review-decision handling now ships as informational downstream context")
    );
}

#[test]
fn checked_in_demo_json_matches_published_schemas() {
    let scan_schema: Value = serde_json::from_str(SCAN_SCHEMA).expect("scan schema json");
    let gate_schema: Value = serde_json::from_str(GATE_SCHEMA).expect("gate schema json");
    let scan_instance: Value = serde_json::from_str(DEMO_SCAN).expect("demo scan instance");
    let gate_instance: Value = serde_json::from_str(DEMO_GATE).expect("demo gate instance");

    assert!(
        meta::is_valid(&scan_schema),
        "scan schema must be valid json schema"
    );
    assert!(
        meta::is_valid(&gate_schema),
        "gate schema must be valid json schema"
    );

    validator_for(&scan_schema)
        .expect("compile scan schema")
        .validate(&scan_instance)
        .expect("demo scan should validate against published schema");
    validator_for(&gate_schema)
        .expect("compile gate schema")
        .validate(&gate_instance)
        .expect("demo gate should validate against published schema");
}

fn demo_traceable_ids(gate: &GateArtifact) -> BTreeSet<&str> {
    let mut ids = BTreeSet::new();
    for blocker in &gate.residual_blockers {
        ids.insert(blocker.comment_id.as_str());
    }
    for blocker in &gate.candidate_blockers {
        ids.insert(blocker.comment_id.as_str());
    }
    for comment in &gate.classified_comments {
        ids.insert(comment.comment.comment_id.as_str());
    }
    ids
}

fn derive_false_residual_rate(corpus: &ManualLabelCorpus) -> Option<f64> {
    let residual_rows = corpus
        .cases
        .iter()
        .filter(|case| case.observed_bucket == ObservedBucket::ResidualBlocker)
        .count();
    if residual_rows == 0 {
        return None;
    }

    let false_residual_rows = corpus
        .cases
        .iter()
        .filter(|case| {
            case.observed_bucket == ObservedBucket::ResidualBlocker
                && case.manual_label != ManualLabel::TrueBlocker
        })
        .count();
    Some(false_residual_rows as f64 / residual_rows as f64)
}

fn derive_missed_obvious_blocker_rate(corpus: &ManualLabelCorpus) -> Option<f64> {
    let obvious_rows = corpus
        .cases
        .iter()
        .filter(|case| case.obvious_blocker)
        .count();
    if obvious_rows == 0 {
        return None;
    }

    let missed_rows = corpus
        .cases
        .iter()
        .filter(|case| {
            case.obvious_blocker && case.observed_bucket != ObservedBucket::ResidualBlocker
        })
        .count();
    Some(missed_rows as f64 / obvious_rows as f64)
}

fn assert_rate_matches(actual: f64, expected: f64, metric_name: &str) {
    assert!(
        actual.to_bits() == expected.to_bits(),
        "{metric_name} should match the checked-in corpus exactly: actual={actual}, expected={expected}"
    );
}
