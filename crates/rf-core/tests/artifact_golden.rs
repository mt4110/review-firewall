use rf_core::domain::{
    AdvisoryWeight, BlockerConcern, DataCoverage, DraftReplyArtifact, EvidenceClass, GateArtifact,
    GateConfigSnapshot, GateCounts, PullRequestSummary, ReplyType, ResidualBlocker, ReviewSignal,
    ScanArtifact, SourceCoverageArtifact, SourceCoverageEntry, SourceCoverageName,
    SourceCoverageStatus, SourceFailureReason, Status, derive_data_coverage_from_sources,
};

#[test]
fn scan_artifact_serializes_minimum_shape() {
    let scan = ScanArtifact {
        status: Status::Ok,
        data_coverage: DataCoverage::Full,
        review_signal: ReviewSignal::Unknown,
        reason: None,
        scan_partial: false,
        repo_root: Some(String::from("/tmp/review-firewall")),
        branch: Some(String::from("feature/test")),
        pr: PullRequestSummary {
            number: Some(142),
            title: String::from("Refactor response handling"),
            ..PullRequestSummary::default()
        },
        files_changed: 8,
        review_comments: 17,
        threads: 6,
        codeowners_found: true,
        policy_found: true,
        product_boundary: Default::default(),
        changed_files: Vec::new(),
        comments: Vec::new(),
        issue_comments: Vec::new(),
        review_threads: Vec::new(),
        partial_sources: Vec::new(),
        warnings: Vec::new(),
    };

    let rendered = serde_json::to_string_pretty(&scan).expect("scan json");
    assert!(rendered.contains(r#""status": "OK""#));
    assert!(rendered.contains(r#""data_coverage": "FULL""#));
    assert!(rendered.contains(r#""review_signal": "UNKNOWN""#));
    assert!(rendered.contains(r#""files_changed": 8"#));
    assert!(rendered.contains(r#""review_comments": 17"#));
    assert!(rendered.contains(r#""threads": 6"#));
    assert!(rendered.contains(r#""codeowners_found": true"#));
    assert!(rendered.contains(r#""policy_found": true"#));
    assert!(rendered.contains(r#""category": "post_review_triage_firewall""#));
    assert!(rendered.contains(r#""generates_ai_reviews": false"#));
    assert!(rendered.contains(r#""posts_pr_comments": false"#));
}

#[test]
fn gate_artifact_serializes_minimum_shape() {
    let gate = GateArtifact {
        status: Status::Ok,
        data_coverage: DataCoverage::Full,
        review_signal: ReviewSignal::Blocked,
        reason: None,
        comments_analyzed: 17,
        residual_blockers: vec![ResidualBlocker {
            comment_id: String::from("12"),
            concern: BlockerConcern::Correctness,
            failure_mode: String::from("partial status may break response contract"),
            evidence_class: EvidenceClass::ContractDelta,
            evidence: vec![String::from(
                "response contract changes when status=partial",
            )],
            owner_match: true,
            ownership_scope: rf_core::domain::OwnershipScope::Exact,
            advisory_weight: AdvisoryWeight::High,
            path: None,
            author: String::from("reviewer"),
        }],
        counts: GateCounts {
            questions: 4,
            suggestions: 5,
            nits: 4,
            praise: 2,
            unknown: 0,
        },
        candidate_blockers: Vec::new(),
        downgraded_comments: Vec::new(),
        duplicates_collapsed: Vec::new(),
        warnings: Vec::new(),
        config_snapshot: GateConfigSnapshot::default(),
        review_decision_summary: None,
        classified_comments: Vec::new(),
        escalation_candidates: Vec::new(),
    };

    let rendered = serde_json::to_string_pretty(&gate).expect("gate json");
    assert!(rendered.contains(r#""data_coverage": "FULL""#));
    assert!(rendered.contains(r#""review_signal": "BLOCKED""#));
    assert!(rendered.contains(r#""evidence_class": "contract_delta""#));
    assert!(rendered.contains(r#""comments_analyzed": 17"#));
    assert!(rendered.contains(r#""residual_blockers""#));
    assert!(rendered.contains(r#""questions": 4"#));
}

#[test]
fn source_coverage_artifact_serializes_minimum_shape() {
    let sources = vec![
        SourceCoverageEntry::new(
            SourceCoverageName::PrMetadata,
            true,
            SourceCoverageStatus::Full,
            1,
            None,
            None,
        ),
        SourceCoverageEntry::new(
            SourceCoverageName::ReviewComments,
            true,
            SourceCoverageStatus::Partial,
            100,
            Some(SourceFailureReason::PaginationPartial),
            Some(String::from(
                "GitHub pagination stopped after page 1 while fetching review comments.",
            )),
        ),
    ];
    let artifact = SourceCoverageArtifact {
        status: Status::Partial,
        data_coverage: derive_data_coverage_from_sources(&sources),
        review_signal: ReviewSignal::Unknown,
        reason: Some(String::from("review comments were only partially observed")),
        sources,
        warnings: Vec::new(),
    };

    let rendered = serde_json::to_string_pretty(&artifact).expect("source coverage json");
    assert!(rendered.contains(r#""data_coverage": "PARTIAL""#));
    assert!(rendered.contains(r#""name": "review_comments""#));
    assert!(rendered.contains(r#""failure_reason": "pagination_partial""#));
    assert!(rendered.contains(r#""retry_hint":"#) || rendered.contains(r#""retry_hint": "#));
}

#[test]
fn draft_reply_roundtrip_is_stable() {
    let draft = DraftReplyArtifact {
        status: Status::Ok,
        data_coverage: DataCoverage::Full,
        review_signal: ReviewSignal::Blocked,
        reason: None,
        reply_type: ReplyType::Accept,
        target_comment_id: Some(String::from("12")),
        body: String::from(
            "Thanks. I agree this is a correctness issue in this PR.\nI will address it here by updating the contract handling.",
        ),
    };

    let rendered = serde_json::to_string_pretty(&draft).expect("draft json");
    assert!(rendered.contains(r#""reply_type": "accept""#));
    assert!(rendered.contains(r#""review_signal": "BLOCKED""#));
    let restored: DraftReplyArtifact =
        serde_json::from_str(&rendered).expect("draft reply roundtrip");
    assert_eq!(restored.body, draft.body);
}

#[test]
fn draft_reply_deserializes_legacy_reply_types() {
    let legacy_decline = r#"{
        "status": "OK",
        "reply_type": "decline",
        "target_comment_id": "12",
        "body": "Thanks. I do not think this blocks merge for this PR."
    }"#;

    let restored: DraftReplyArtifact =
        serde_json::from_str(legacy_decline).expect("legacy decline draft");
    assert_eq!(restored.reply_type, ReplyType::CannotClassify);

    let legacy_move = r#"{
        "status": "OK",
        "reply_type": "move",
        "target_comment_id": "12",
        "body": "Thanks. I propose moving this to ADR/RFC."
    }"#;

    let restored: DraftReplyArtifact =
        serde_json::from_str(legacy_move).expect("legacy move draft");
    assert_eq!(restored.reply_type, ReplyType::MoveToAdr);
}

#[test]
fn gate_artifact_deserializes_legacy_residual_blockers_without_evidence_class() {
    let legacy_gate = r#"{
        "status": "OK",
        "comments_analyzed": 1,
        "residual_blockers": [
            {
                "comment_id": "12",
                "concern": "correctness",
                "failure_mode": "partial status may break response contract",
                "evidence": ["response contract changes when status=partial"],
                "owner_match": true,
                "ownership_scope": "exact",
                "advisory_weight": "high",
                "author": "reviewer-a"
            }
        ],
        "counts": {
            "questions": 0,
            "suggestions": 0,
            "nits": 0,
            "praise": 0
        },
        "config_snapshot": {
            "require_failure_mode": true,
            "require_concern": true,
            "require_evidence": true,
            "require_alternative": false,
            "max_pr_thread_roundtrips": 2,
            "use_codeowners": true
        }
    }"#;

    let restored: GateArtifact = serde_json::from_str(legacy_gate).expect("legacy gate");
    assert_eq!(restored.residual_blockers.len(), 1);
    assert_eq!(
        restored.residual_blockers[0].evidence_class,
        EvidenceClass::ConcreteReference
    );
}

#[test]
fn scan_artifact_deserializes_legacy_partial_scan_as_partial_coverage() {
    let legacy_scan = r#"{
        "status": "OK",
        "scan_partial": true,
        "pr": {
            "number": 2,
            "title": "Legacy partial scan"
        },
        "files_changed": 3,
        "review_comments": 5,
        "threads": 2,
        "codeowners_found": false,
        "policy_found": true,
        "partial_sources": ["gh"]
    }"#;

    let restored: ScanArtifact = serde_json::from_str(legacy_scan).expect("legacy partial scan");
    assert_eq!(restored.status, Status::Ok);
    assert_eq!(restored.data_coverage, DataCoverage::Partial);
    assert_eq!(restored.review_signal, ReviewSignal::Unknown);
}

#[test]
fn scan_artifact_deserializes_legacy_error_scan_as_failed_coverage() {
    let legacy_scan = r#"{
        "status": "ERROR",
        "pr": {
            "number": 2,
            "title": "Legacy error scan"
        },
        "files_changed": 0,
        "review_comments": 0,
        "threads": 0,
        "codeowners_found": false,
        "policy_found": false
    }"#;

    let restored: ScanArtifact = serde_json::from_str(legacy_scan).expect("legacy error scan");
    assert_eq!(restored.status, Status::Error);
    assert_eq!(restored.data_coverage, DataCoverage::Failed);
    assert_eq!(restored.review_signal, ReviewSignal::Unknown);
}

#[test]
fn scan_artifact_deserializes_legacy_complete_scan_as_full_coverage() {
    let legacy_scan = r#"{
        "status": "OK",
        "scan_partial": false,
        "pr": {
            "number": 2,
            "title": "Legacy complete scan"
        },
        "files_changed": 3,
        "review_comments": 5,
        "threads": 2,
        "codeowners_found": true,
        "policy_found": true
    }"#;

    let restored: ScanArtifact = serde_json::from_str(legacy_scan).expect("legacy full scan");
    assert_eq!(restored.status, Status::Ok);
    assert_eq!(restored.data_coverage, DataCoverage::Full);
    assert_eq!(restored.review_signal, ReviewSignal::Unknown);
}

#[test]
fn source_coverage_derives_failed_when_required_source_fails() {
    let sources = vec![
        SourceCoverageEntry::new(
            SourceCoverageName::PrMetadata,
            true,
            SourceCoverageStatus::Failed,
            0,
            Some(SourceFailureReason::GhNotAuthenticated),
            Some(String::from("GitHub CLI is not authenticated.")),
        ),
        SourceCoverageEntry::new(
            SourceCoverageName::Codeowners,
            false,
            SourceCoverageStatus::Partial,
            0,
            None,
            Some(String::from("CODEOWNERS could not be read.")),
        ),
    ];

    assert_eq!(
        derive_data_coverage_from_sources(&sources),
        DataCoverage::Failed
    );
}
