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
    let scan = base_scan(
        "This PR can Break consumers Because status=partial changes the response shape in this PR.",
    );

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
            .any(|value| value.contains("Because status=partial changes the response shape")),
        "evidence should preserve original reviewer text"
    );
}

#[test]
fn broad_contract_wording_without_specific_delta_becomes_keyword_only() {
    let scan =
        base_scan("This can break consumers in this PR because the response contract changes.");

    let gate = gate_scan(&scan, &GateConfigSnapshot::default(), &[]);

    let comment = gate
        .classified_comments
        .first()
        .expect("classified comment");
    assert!(gate.residual_blockers.is_empty());
    assert!(comment.evidence.is_empty());
    assert_eq!(comment.evidence_class, Some(EvidenceClass::KeywordOnly));
}

#[test]
fn metalinguistic_failure_mode_extractor_is_not_treated_as_runtime_failure_mode() {
    let scan =
        base_scan("The failure-mode extractor wording could be narrower in the docs for this PR.");

    let gate = gate_scan(&scan, &GateConfigSnapshot::default(), &[]);

    let comment = gate
        .classified_comments
        .first()
        .expect("classified comment");
    assert_eq!(
        comment.comment_type,
        rf_core::domain::CommentType::Suggestion
    );
    assert_eq!(comment.failure_mode, None);
    assert!(gate.residual_blockers.is_empty());
}

#[test]
fn metalinguistic_failure_mode_wrong_wording_stays_non_runtime() {
    let scan = base_scan("The failure-mode extractor wording is wrong in the docs for this PR.");

    let gate = gate_scan(&scan, &GateConfigSnapshot::default(), &[]);

    let comment = gate
        .classified_comments
        .first()
        .expect("classified comment");
    assert_eq!(
        comment.comment_type,
        rf_core::domain::CommentType::Suggestion
    );
    assert_eq!(comment.failure_mode, None);
    assert!(gate.residual_blockers.is_empty());
}

#[test]
fn metalinguistic_failure_mode_docs_fail_to_explain_stays_non_runtime() {
    let scan =
        base_scan("The failure-mode extractor docs fail to explain the narrower wording here.");

    let gate = gate_scan(&scan, &GateConfigSnapshot::default(), &[]);

    let comment = gate
        .classified_comments
        .first()
        .expect("classified comment");
    assert_eq!(
        comment.comment_type,
        rf_core::domain::CommentType::Suggestion
    );
    assert_eq!(comment.failure_mode, None);
    assert!(gate.residual_blockers.is_empty());
}

#[test]
fn metalinguistic_failure_mode_docs_fail_to_explain_stays_non_runtime_when_relaxed() {
    let scan =
        base_scan("The failure-mode extractor docs fail to explain the narrower wording here.");
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
    assert_ne!(comment.comment_type, rf_core::domain::CommentType::Blocker);
    assert_eq!(comment.concern, None);
    assert_eq!(comment.failure_mode, None);
    assert!(!comment.present_pr_impact);
    assert!(gate.residual_blockers.is_empty());
}

#[test]
fn metalinguistic_comment_with_runtime_crash_still_extracts_failure_mode() {
    let scan = base_scan(
        "The failure-mode extractor documentation is wrong because it crashes on empty threads.",
    );

    let gate = gate_scan(&scan, &GateConfigSnapshot::default(), &[]);

    let comment = gate
        .classified_comments
        .first()
        .expect("classified comment");
    assert!(
        comment
            .failure_mode
            .as_deref()
            .is_some_and(|value| value.contains("crashes on empty threads")),
        "runtime crash details should survive even when wording/docs are mentioned"
    );
}

#[test]
fn metalinguistic_comment_with_runtime_fail_context_still_extracts_failure_mode() {
    let scan = base_scan(
        "The failure-mode extractor documentation is wrong because it fails in long-term retry loops.",
    );

    let gate = gate_scan(&scan, &GateConfigSnapshot::default(), &[]);

    let comment = gate
        .classified_comments
        .first()
        .expect("classified comment");
    assert!(
        comment
            .failure_mode
            .as_deref()
            .is_some_and(|value| value.contains("fails in long-term retry loops")),
        "runtime fail-context details should survive even when wording/docs are mentioned"
    );
}

#[test]
fn metalinguistic_comment_with_plain_runtime_fails_still_extracts_failure_mode() {
    let scan =
        base_scan("The failure-mode extractor docs are wrong because it fails after retries.");

    let gate = gate_scan(&scan, &GateConfigSnapshot::default(), &[]);

    let comment = gate
        .classified_comments
        .first()
        .expect("classified comment");
    assert!(
        comment
            .failure_mode
            .as_deref()
            .is_some_and(|value| value.contains("it fails after retries")),
        "plain runtime fail verbs should survive even when wording/docs are mentioned"
    );
}

#[test]
fn metalinguistic_comment_with_plain_runtime_fails_and_repro_still_blocks() {
    let scan = base_scan(
        "The failure-mode extractor docs are wrong in this PR because it fails after retries when the same request is replayed under the retry loop.",
    );

    let gate = gate_scan(&scan, &GateConfigSnapshot::default(), &[]);

    let blocker = gate.residual_blockers.first().expect("residual blocker");
    assert_eq!(blocker.evidence_class, EvidenceClass::ReproCondition);
    assert!(
        blocker.failure_mode.contains("fails after retries"),
        "plain runtime fail verbs should still reach author-facing blockers when repro evidence is present"
    );
    assert!(
        blocker
            .evidence
            .iter()
            .any(|value| value.contains("when the same request is replayed")),
        "repro evidence should survive alongside recovered plain-fail failure modes"
    );
}

#[test]
fn metalinguistic_comment_with_runtime_failure_noun_still_extracts_failure_mode() {
    let scan = base_scan(
        "The failure-mode extractor documentation is wrong because it causes a failure in rollback handling.",
    );

    let gate = gate_scan(&scan, &GateConfigSnapshot::default(), &[]);

    let comment = gate
        .classified_comments
        .first()
        .expect("classified comment");
    assert!(
        comment
            .failure_mode
            .as_deref()
            .is_some_and(|value| value.contains("failure in rollback handling")),
        "runtime failure-noun details should survive even when wording/docs are mentioned"
    );
}

#[test]
fn metalinguistic_comment_with_runtime_failure_to_still_extracts_failure_mode() {
    let scan = base_scan(
        "The failure-mode extractor docs are wrong because it causes failure to parse rollback metadata.",
    );

    let gate = gate_scan(&scan, &GateConfigSnapshot::default(), &[]);

    let comment = gate
        .classified_comments
        .first()
        .expect("classified comment");
    assert!(
        comment
            .failure_mode
            .as_deref()
            .is_some_and(|value| value.contains("failure to parse rollback metadata")),
        "runtime failure-to details should survive even when wording/docs are mentioned"
    );
}

#[test]
fn metalinguistic_comment_with_runtime_causes_failure_still_extracts_failure_mode() {
    let scan = base_scan("The failure-mode extractor docs are wrong because it causes failure.");

    let gate = gate_scan(&scan, &GateConfigSnapshot::default(), &[]);

    let comment = gate
        .classified_comments
        .first()
        .expect("classified comment");
    assert!(
        comment
            .failure_mode
            .as_deref()
            .is_some_and(|value| value.contains("causes failure")),
        "runtime causes-failure details should survive even when wording/docs are mentioned"
    );
}

#[test]
fn metalinguistic_comment_with_incorrect_output_still_extracts_failure_mode() {
    let scan = base_scan(
        "The failure-mode extractor docs are wrong because the classification output is incorrect for clean runs.",
    );

    let gate = gate_scan(&scan, &GateConfigSnapshot::default(), &[]);

    let comment = gate
        .classified_comments
        .first()
        .expect("classified comment");
    assert!(
        comment
            .failure_mode
            .as_deref()
            .is_some_and(|value| value.contains("classification output is incorrect")),
        "runtime incorrect-output details should survive even when wording/docs are mentioned"
    );
}

#[test]
fn runtime_causes_failure_still_extracts_failure_mode() {
    let scan = base_scan("This change causes failure when rollback metadata is missing.");

    let gate = gate_scan(&scan, &GateConfigSnapshot::default(), &[]);

    let comment = gate
        .classified_comments
        .first()
        .expect("classified comment");
    assert!(
        comment
            .failure_mode
            .as_deref()
            .is_some_and(|value| value.contains("causes failure")),
        "runtime causes-failure phrasing should remain a failure-mode signal"
    );
}

#[test]
fn runtime_failure_noun_still_extracts_failure_mode() {
    let scan =
        base_scan("This PR introduces a failure in rollback handling when retries are enabled.");

    let gate = gate_scan(&scan, &GateConfigSnapshot::default(), &[]);

    let comment = gate
        .classified_comments
        .first()
        .expect("classified comment");
    assert!(
        comment
            .failure_mode
            .as_deref()
            .is_some_and(|value| value.contains("failure in rollback handling")),
        "runtime failure-noun phrasing should remain a failure-mode signal"
    );
}

#[test]
fn runtime_failure_to_still_extracts_failure_mode() {
    let scan = base_scan("This PR causes failure to parse rollback metadata when retries begin.");

    let gate = gate_scan(&scan, &GateConfigSnapshot::default(), &[]);

    let comment = gate
        .classified_comments
        .first()
        .expect("classified comment");
    assert!(
        comment
            .failure_mode
            .as_deref()
            .is_some_and(|value| value.contains("failure to parse rollback metadata")),
        "runtime failure-to phrasing should remain a failure-mode signal"
    );
}

#[test]
fn docker_word_does_not_trigger_doc_meta_filter_by_substring() {
    let scan = base_scan(
        "This PR breaks failure-mode extractor behavior in Docker jobs, so true blockers are dropped.",
    );

    let gate = gate_scan(&scan, &GateConfigSnapshot::default(), &[]);

    let comment = gate
        .classified_comments
        .first()
        .expect("classified comment");
    assert!(
        comment
            .failure_mode
            .as_deref()
            .is_some_and(|value| value.contains("Docker jobs")),
        "docker should not be mistaken for a doc/docs wording marker"
    );
}

#[test]
fn classifier_breakage_comment_still_extracts_failure_mode() {
    let scan = base_scan(
        "This PR breaks failure-mode matching for partial status, so true blockers are dropped.",
    );

    let gate = gate_scan(&scan, &GateConfigSnapshot::default(), &[]);

    let comment = gate
        .classified_comments
        .first()
        .expect("classified comment");
    assert_eq!(
        comment.failure_mode.as_deref(),
        Some(
            "This PR breaks failure-mode matching for partial status, so true blockers are dropped"
        )
    );
}

#[test]
fn inflected_failure_verbs_still_extract_failure_mode() {
    for (body, expected_fragment) in [
        (
            "The failure-mode extractor is failing in Docker jobs, so true blockers are dropped.",
            "failing in Docker jobs",
        ),
        (
            "This change regressed failure-mode matching for partial status, so true blockers are dropped.",
            "regressed failure-mode matching",
        ),
        (
            "The current branch introduces failure-mode breakage in partial scans, so true blockers are dropped.",
            "failure-mode breakage in partial scans",
        ),
    ] {
        let scan = base_scan(body);

        let gate = gate_scan(&scan, &GateConfigSnapshot::default(), &[]);

        let comment = gate
            .classified_comments
            .first()
            .expect("classified comment");
        assert!(
            comment
                .failure_mode
                .as_deref()
                .is_some_and(|value| value.contains(expected_fragment)),
            "expected failure_mode to retain inflected runtime signal for: {body}"
        );
    }
}

#[test]
fn operational_failure_path_still_counts_as_runtime_failure_mode() {
    let scan = base_scan(
        "This PR adds a new failure path without `metrics`, so failures will not be detected during rollout.",
    );

    let gate = gate_scan(&scan, &GateConfigSnapshot::default(), &[]);

    let blocker = gate.residual_blockers.first().expect("residual blocker");
    assert_eq!(blocker.concern, BlockerConcern::Operability);
    assert!(
        blocker
            .failure_mode
            .contains("failures will not be detected"),
        "real failure-path comments should still extract a concrete failure mode"
    );
}

#[test]
fn auth_runtime_sentence_is_not_filtered_as_contract_only() {
    let scan = base_scan(
        "This can leak the auth token in this PR because when this request returns 500, clients retry with the same secret.",
    );

    let gate = gate_scan(&scan, &GateConfigSnapshot::default(), &[]);

    let blocker = gate.residual_blockers.first().expect("residual blocker");
    assert_eq!(blocker.concern, BlockerConcern::Security);
    assert_eq!(blocker.evidence_class, EvidenceClass::SecurityCondition);
    assert!(
        blocker
            .evidence
            .iter()
            .any(|value| value.contains("auth token")),
        "runtime-specific evidence should survive broad contract filtering"
    );
}

#[test]
fn credential_leak_sentence_is_not_filtered_as_contract_only() {
    let scan = base_scan(
        "This can leak credentials in this PR because the response contract changes for unauthenticated clients.",
    );

    let gate = gate_scan(&scan, &GateConfigSnapshot::default(), &[]);

    let blocker = gate.residual_blockers.first().expect("residual blocker");
    assert_eq!(blocker.concern, BlockerConcern::Security);
    assert!(
        blocker
            .evidence
            .iter()
            .any(|value| value.contains("leak credentials")),
        "non-contract security impact should survive broad contract filtering"
    );
}

#[test]
fn contract_delta_with_array_to_object_survives_filter() {
    let scan = base_scan(
        "This can break consumers in this PR because the response changes from array to object.",
    );

    let gate = gate_scan(&scan, &GateConfigSnapshot::default(), &[]);

    let blocker = gate.residual_blockers.first().expect("residual blocker");
    assert_eq!(blocker.concern, BlockerConcern::Correctness);
    assert_eq!(blocker.evidence_class, EvidenceClass::ContractDelta);
    assert!(
        blocker
            .evidence
            .iter()
            .any(|value| value.contains("array to object")),
        "concrete contract delta wording should survive the contract-only filter"
    );
}

#[test]
fn unrelated_backtick_reference_does_not_restore_contract_delta() {
    let scan = base_scan(
        "This can break consumers in this PR because the response contract changes around `foo`.",
    );

    let gate = gate_scan(&scan, &GateConfigSnapshot::default(), &[]);

    let comment = gate
        .classified_comments
        .first()
        .expect("classified comment");
    assert!(gate.residual_blockers.is_empty());
    assert_eq!(comment.evidence_class, Some(EvidenceClass::KeywordOnly));
}

#[test]
fn short_concrete_backtick_identifier_can_satisfy_evidence_requirement() {
    let scan = base_scan("This can leak credentials in this PR around `jwt`.");

    let gate = gate_scan(&scan, &GateConfigSnapshot::default(), &[]);

    let blocker = gate.residual_blockers.first().expect("residual blocker");
    assert_eq!(blocker.concern, BlockerConcern::Security);
    assert_eq!(blocker.evidence_class, EvidenceClass::ConcreteReference);
    assert!(
        blocker.evidence.iter().any(|value| value.contains("`jwt`")),
        "short concrete identifiers should still count as backtick evidence"
    );
}

#[test]
fn placeholder_backtick_reference_does_not_satisfy_evidence_requirement() {
    let scan = base_scan(
        "This can break consumers in this PR because the response contract changes around `TODO`.",
    );

    let gate = gate_scan(&scan, &GateConfigSnapshot::default(), &[]);

    let comment = gate
        .classified_comments
        .first()
        .expect("classified comment");
    assert!(gate.residual_blockers.is_empty());
    assert!(comment.evidence.is_empty());
    assert_eq!(comment.evidence_class, Some(EvidenceClass::KeywordOnly));
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
fn evidence_class_does_not_match_author_as_auth() {
    let scan = base_scan(
        "This can break the response contract in this PR because the author changed the response shape.",
    );
    let config = GateConfigSnapshot {
        require_evidence: false,
        ..GateConfigSnapshot::default()
    };

    let gate = gate_scan(&scan, &config, &[]);
    let blocker = gate.residual_blockers.first().expect("residual blocker");

    assert_ne!(blocker.evidence_class, EvidenceClass::SecurityCondition);
    assert_eq!(blocker.evidence_class, EvidenceClass::ContractDelta);
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

#[test]
fn changes_requested_review_state_stays_informational_when_gate_is_clear() {
    let mut scan = base_scan("Could we rename this helper for clarity?");
    scan.pr.review_decisions = vec![String::from("CHANGES_REQUESTED")];

    let gate = gate_scan(&scan, &GateConfigSnapshot::default(), &[]);
    let summary = gate
        .review_decision_summary
        .as_ref()
        .expect("review decision summary");

    assert_eq!(gate.review_signal, ReviewSignal::Clear);
    assert!(summary.changes_requested);
    assert!(summary.informational_only);
}
