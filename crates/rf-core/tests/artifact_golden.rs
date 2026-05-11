use rf_core::domain::{
    AdvisoryWeight, BlockerConcern, DataCoverage, DraftReplyArtifact, EvidenceClass, GateArtifact,
    GateConfigSnapshot, GateCounts, PullRequestSummary, ReplyType, ResidualBlocker, ReviewSignal,
    ScanArtifact, Status,
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
