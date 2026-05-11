use rf_core::domain::{
    BlockerConcern, CommentRecord, CommentSource, DataCoverage, EvidenceClass, GateConfigSnapshot,
    PullRequestSummary, ReviewSignal, ScanArtifact, Status,
};
use rf_core::{build_review_threads, gate_scan};

fn base_scan(comment_body: &str) -> ScanArtifact {
    let comment = CommentRecord {
        comment_id: String::from("12"),
        thread_id: String::from("12"),
        author: String::from("reviewer-a"),
        body: String::from(comment_body),
        path: Some(String::from("src/api/response.rs")),
        source: CommentSource::ReviewComment,
        reply_to_comment_id: None,
        created_at: Some(String::from("2026-03-28T00:00:00Z")),
        line: Some(10),
        original_line: Some(10),
    };

    ScanArtifact {
        status: Status::Ok,
        data_coverage: DataCoverage::Full,
        review_signal: ReviewSignal::Unknown,
        reason: None,
        scan_partial: false,
        repo_root: Some(String::from("/tmp/review-firewall")),
        branch: Some(String::from("feature/test")),
        pr: PullRequestSummary {
            number: Some(42),
            title: String::from("Refactor response handling"),
            ..PullRequestSummary::default()
        },
        files_changed: 1,
        review_comments: 1,
        threads: 1,
        codeowners_found: false,
        policy_found: false,
        product_boundary: Default::default(),
        changed_files: vec![String::from("src/api/response.rs")],
        comments: vec![comment.clone()],
        issue_comments: Vec::new(),
        review_threads: build_review_threads(&[comment]),
        partial_sources: Vec::new(),
        warnings: Vec::new(),
    }
}

#[test]
fn changed_path_alone_is_not_evidence() {
    let scan = base_scan("This looks risky.");
    let gate = gate_scan(&scan, &GateConfigSnapshot::default(), &[]);

    assert!(gate.residual_blockers.is_empty());
    assert!(
        gate.classified_comments
            .first()
            .expect("classified comment")
            .evidence
            .is_empty()
    );
}

#[test]
fn changed_path_alone_is_not_present_pr_impact() {
    let scan = base_scan("This can break consumers.");
    let gate = gate_scan(&scan, &GateConfigSnapshot::default(), &[]);

    let comment = gate
        .classified_comments
        .first()
        .expect("classified comment");
    assert!(!comment.present_pr_impact);
    assert!(gate.residual_blockers.is_empty());
}

#[test]
fn pr_scope_markers_require_word_boundaries() {
    let scan = base_scan(
        "This can break consumers where emergency handling fails because response contract changes.",
    );

    let gate = gate_scan(&scan, &GateConfigSnapshot::default(), &[]);

    let comment = gate
        .classified_comments
        .first()
        .expect("classified comment");
    assert!(!comment.present_pr_impact);
    assert!(gate.residual_blockers.is_empty());
}

#[test]
fn failure_mode_and_evidence_detection_is_case_insensitive() {
    let scan =
        base_scan("This PR can Break consumers Because response contract changes in this PR.");

    let gate = gate_scan(&scan, &GateConfigSnapshot::default(), &[]);

    let blocker = gate.residual_blockers.first().expect("residual blocker");
    assert!(
        blocker.failure_mode.contains("Break consumers"),
        "failure mode should preserve original reviewer text"
    );
    assert!(
        blocker
            .evidence
            .iter()
            .any(|value| value.contains("Because response contract")),
        "evidence should preserve original reviewer text"
    );
}

#[test]
fn scoped_concern_can_become_present_pr_impact_when_requirements_are_disabled() {
    let scan = base_scan("This security concern is here in this PR.");
    let config = GateConfigSnapshot {
        require_failure_mode: false,
        require_evidence: false,
        ..GateConfigSnapshot::default()
    };

    let gate = gate_scan(&scan, &config, &[]);

    let comment = gate
        .classified_comments
        .first()
        .expect("classified comment");
    let blocker = gate.residual_blockers.first().expect("residual blocker");
    assert!(comment.present_pr_impact);
    assert_eq!(blocker.concern, BlockerConcern::Security);
    assert_eq!(blocker.evidence_class, EvidenceClass::KeywordOnly);
}

#[test]
fn config_can_disable_failure_mode_requirement_when_impact_is_evidenced() {
    let scan =
        base_scan("This PR has a security concern because the response includes the auth token.");
    let config = GateConfigSnapshot {
        require_failure_mode: false,
        ..GateConfigSnapshot::default()
    };

    let gate = gate_scan(&scan, &config, &[]);

    let blocker = gate.residual_blockers.first().expect("residual blocker");
    assert_eq!(blocker.concern, BlockerConcern::Security);
    assert_eq!(blocker.failure_mode, "failure mode was not extracted");
}

#[test]
fn config_can_disable_evidence_requirement() {
    let scan = base_scan(
        "This can break the response contract in this PR and should be fixed before merge.",
    );
    let config = GateConfigSnapshot {
        require_evidence: false,
        ..GateConfigSnapshot::default()
    };

    let gate = gate_scan(&scan, &config, &[]);

    let comment = gate
        .classified_comments
        .first()
        .expect("classified comment");
    let blocker = gate.residual_blockers.first().expect("residual blocker");
    assert_eq!(blocker.concern, BlockerConcern::Correctness);
    assert_eq!(blocker.evidence_class, EvidenceClass::KeywordOnly);
    assert!(comment.evidence.is_empty());
    assert_eq!(
        blocker.evidence,
        vec![String::from("evidence was not extracted")]
    );
}

#[test]
fn failure_mode_text_is_not_reused_as_evidence() {
    let scan = base_scan("This can break consumers in this PR.");
    let config = GateConfigSnapshot {
        require_evidence: false,
        ..GateConfigSnapshot::default()
    };

    let gate = gate_scan(&scan, &config, &[]);
    let comment = gate
        .classified_comments
        .first()
        .expect("classified comment");
    let blocker = gate.residual_blockers.first().expect("residual blocker");

    assert_eq!(
        comment.failure_mode.as_deref(),
        Some("This can break consumers in this PR")
    );
    assert!(comment.evidence.is_empty());
    assert_eq!(blocker.evidence_class, EvidenceClass::KeywordOnly);
}

#[test]
fn config_rejects_comment_without_required_alternative() {
    let scan = base_scan(
        "This can break the response contract in this PR because `partial` changes client handling.",
    );
    let config = GateConfigSnapshot {
        require_alternative: true,
        ..GateConfigSnapshot::default()
    };

    let gate = gate_scan(&scan, &config, &[]);

    assert!(gate.residual_blockers.is_empty());
}

#[test]
fn config_accepts_comment_with_required_alternative() {
    let scan = base_scan(
        "This can break the response contract in this PR because `partial` changes client handling. Instead, keep the current response shape until the schema migration is ready.",
    );
    let config = GateConfigSnapshot {
        require_alternative: true,
        ..GateConfigSnapshot::default()
    };

    let gate = gate_scan(&scan, &config, &[]);

    assert_eq!(gate.residual_blockers.len(), 1);
}

#[test]
fn config_rejects_generic_fix_word_when_alternative_is_required() {
    let scan = base_scan(
        "This can break the response contract in this PR because `partial` changes client handling. This should be fixed before merge.",
    );
    let config = GateConfigSnapshot {
        require_alternative: true,
        ..GateConfigSnapshot::default()
    };

    let gate = gate_scan(&scan, &config, &[]);

    assert!(gate.residual_blockers.is_empty());
}

#[test]
fn author_replies_are_matched_case_insensitively() {
    let mut scan = base_scan(
        "Fixed in this PR. Added a regression test for the merge behavior and should be safe now.",
    );
    scan.pr.author = String::from("Author");
    scan.comments[0].author = String::from("author");
    let config = GateConfigSnapshot {
        require_evidence: false,
        ..GateConfigSnapshot::default()
    };

    let gate = gate_scan(&scan, &config, &[]);

    assert!(gate.residual_blockers.is_empty());
    assert!(gate.candidate_blockers.is_empty());
}

#[test]
fn residual_blockers_collapse_to_one_per_thread() {
    let mut scan = base_scan(
        "This can break the response contract in this PR because `partial` changes client handling.",
    );
    scan.comments.push(CommentRecord {
        comment_id: String::from("13"),
        thread_id: String::from("12"),
        author: String::from("reviewer-a"),
        body: String::from(
            "This can break the response contract in this PR because `partial` changes client handling.",
        ),
        path: Some(String::from("src/api/response.rs")),
        source: CommentSource::ReviewComment,
        reply_to_comment_id: Some(String::from("12")),
        created_at: Some(String::from("2026-03-28T00:00:01Z")),
        line: Some(11),
        original_line: Some(11),
    });
    scan.review_threads = build_review_threads(&scan.comments);

    let gate = gate_scan(&scan, &GateConfigSnapshot::default(), &[]);

    assert_eq!(gate.residual_blockers.len(), 1);
    assert_eq!(gate.candidate_blockers.len(), 1);
    assert_eq!(gate.duplicates_collapsed.len(), 1);
}

#[test]
fn duplicate_author_comment_does_not_hide_reviewer_blocker() {
    let mut scan = base_scan(
        "This can break the response contract in this PR because `partial` changes client handling.",
    );
    scan.pr.author = String::from("author");
    scan.comments[0].author = String::from("author");
    scan.comments.push(CommentRecord {
        comment_id: String::from("13"),
        thread_id: String::from("12"),
        author: String::from("reviewer-a"),
        body: String::from(
            "This can break the response contract in this PR because `partial` changes client handling.",
        ),
        path: Some(String::from("src/api/response.rs")),
        source: CommentSource::ReviewComment,
        reply_to_comment_id: Some(String::from("12")),
        created_at: Some(String::from("2026-03-28T00:00:01Z")),
        line: Some(11),
        original_line: Some(11),
    });
    scan.review_threads = build_review_threads(&scan.comments);

    let gate = gate_scan(&scan, &GateConfigSnapshot::default(), &[]);

    assert_eq!(gate.residual_blockers.len(), 1);
    assert_eq!(gate.residual_blockers[0].comment_id, "13");
    assert_eq!(gate.candidate_blockers.len(), 1);
    assert_eq!(gate.candidate_blockers[0].comment_id, "13");
    assert_eq!(gate.duplicates_collapsed.len(), 1);
    assert_eq!(gate.duplicates_collapsed[0].primary_comment_id, "13");
    assert_eq!(
        gate.duplicates_collapsed[0].duplicate_comment_ids,
        vec![String::from("12")]
    );
}

#[test]
fn duplicate_detection_preserves_identical_text_in_separate_threads() {
    let mut scan = base_scan(
        "This can break the response contract in this PR because `partial` changes client handling.",
    );
    scan.comments.push(CommentRecord {
        comment_id: String::from("99"),
        thread_id: String::from("99"),
        author: String::from("reviewer-a"),
        body: String::from(
            "This can break the response contract in this PR because `partial` changes client handling.",
        ),
        path: Some(String::from("src/api/response.rs")),
        source: CommentSource::ReviewComment,
        reply_to_comment_id: None,
        created_at: Some(String::from("2026-03-28T00:00:01Z")),
        line: Some(20),
        original_line: Some(20),
    });
    scan.review_threads = build_review_threads(&scan.comments);

    let gate = gate_scan(&scan, &GateConfigSnapshot::default(), &[]);

    assert_eq!(gate.residual_blockers.len(), 2);
    assert_eq!(gate.candidate_blockers.len(), 2);
    assert!(gate.duplicates_collapsed.is_empty());
}

#[test]
fn badge_markup_does_not_leak_into_failure_mode() {
    let scan = base_scan(
        "<sub><img src=\"https://img.shields.io/badge/coverage-100%25-brightgreen?style=flat\" /></sub>\nmode: response contract changes when status=partial.\nbecause clients still parse the old shape.\nin this pr: this PR changes the response shape.",
    );

    let gate = gate_scan(&scan, &GateConfigSnapshot::default(), &[]);
    let blocker = gate.residual_blockers.first().expect("residual blocker");

    assert!(!blocker.failure_mode.contains("style=flat"));
    assert!(!blocker.evidence.join(" ").contains("style=flat"));
}

#[test]
fn badge_only_comment_becomes_noise_only() {
    let scan = base_scan(
        "<sub><img src=\"https://img.shields.io/badge/test-passing?style=flat\" /></sub>",
    );

    let gate = gate_scan(&scan, &GateConfigSnapshot::default(), &[]);
    let comment = gate
        .classified_comments
        .first()
        .expect("classified comment");

    assert_eq!(comment.comment_type, rf_core::domain::CommentType::Unknown);
    assert_eq!(comment.evidence_class, Some(EvidenceClass::NoiseOnly));
    assert!(gate.residual_blockers.is_empty());
}

#[test]
fn normalize_preserves_generic_like_code_text() {
    let normalized = rf_core::normalize::normalize_body(
        "Returning Option<Result<T>> here can break callers in this PR.",
    );

    assert!(normalized.contains("option<result<t"));
}

#[test]
fn partial_scan_keeps_review_signal_unknown() {
    let mut scan = base_scan(
        "This can break the response contract in this PR because `partial` changes client handling.",
    );
    scan.status = Status::Partial;
    scan.data_coverage = DataCoverage::Partial;

    let gate = gate_scan(&scan, &GateConfigSnapshot::default(), &[]);

    assert_eq!(gate.review_signal, ReviewSignal::Unknown);
}
