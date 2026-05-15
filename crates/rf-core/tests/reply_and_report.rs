use rf_core::build_draft_reply;
use rf_core::domain::{
    AdvisoryWeight, BlockerConcern, ClassifiedComment, CommentRecord, CommentSource, CommentType,
    DataCoverage, DraftReplyArtifact, EscalationCandidate, EscalationLabel, EvidenceClass,
    GateArtifact, GateConfigSnapshot, GateCounts, ReplyType, ResidualBlocker,
    ReviewDecisionSummary, ReviewSignal, ScanArtifact, SourceCoverageArtifact, SourceCoverageEntry,
    SourceCoverageName, SourceCoverageStatus, SourceFailureReason, Status,
};
use rf_core::{ReportHeader, ReportInputs, build_report_markdown};

#[test]
fn draft_reply_respects_max_lines() {
    let gate = GateArtifact {
        status: Status::Ok,
        data_coverage: DataCoverage::Full,
        review_signal: ReviewSignal::Blocked,
        reason: None,
        comments_analyzed: 1,
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
            path: Some(String::from("src/api/response.rs")),
            author: String::from("reviewer-a"),
        }],
        counts: GateCounts::default(),
        candidate_blockers: Vec::new(),
        downgraded_comments: Vec::new(),
        duplicates_collapsed: Vec::new(),
        warnings: Vec::new(),
        config_snapshot: GateConfigSnapshot::default(),
        review_decision_summary: None,
        classified_comments: Vec::new(),
        escalation_candidates: Vec::new(),
    };

    let draft = build_draft_reply(&gate, 2);

    assert_eq!(draft.reply_type, ReplyType::Accept);
    assert!(draft.body.lines().count() <= 2);
}

#[test]
fn draft_reply_avoids_merge_judgment_when_gate_is_error() {
    let gate = non_authoritative_gate(Status::Error, "scan.json could not be read");

    let draft = build_draft_reply(&gate, 3);

    assert_eq!(draft.status, Status::Error);
    assert_eq!(draft.reply_type, ReplyType::CannotClassify);
    assert!(draft.target_comment_id.is_none());
    assert!(draft.body.contains("could not complete blocker analysis"));
    assert!(draft.body.contains("scan.json could not be read"));
    assert!(!draft.body.contains("does not think this blocks merge"));
}

#[test]
fn draft_reply_avoids_merge_judgment_when_gate_is_partial() {
    let gate = non_authoritative_gate(
        Status::Partial,
        "review comments were partially unavailable",
    );

    let draft = build_draft_reply(&gate, 3);

    assert_eq!(draft.status, Status::Partial);
    assert_eq!(draft.reply_type, ReplyType::CannotClassify);
    assert!(draft.target_comment_id.is_none());
    assert!(draft.body.contains("could not complete blocker analysis"));
    assert!(
        draft
            .body
            .contains("review comments were partially unavailable")
    );
    assert!(!draft.body.contains("does not think this blocks merge"));
}

#[test]
fn draft_reply_prefers_escalation_before_asking_for_evidence() {
    let gate = GateArtifact {
        status: Status::Ok,
        data_coverage: DataCoverage::Full,
        review_signal: ReviewSignal::Clear,
        reason: None,
        comments_analyzed: 1,
        residual_blockers: Vec::new(),
        counts: GateCounts::default(),
        candidate_blockers: Vec::new(),
        downgraded_comments: Vec::new(),
        duplicates_collapsed: Vec::new(),
        warnings: Vec::new(),
        config_snapshot: GateConfigSnapshot::default(),
        review_decision_summary: None,
        classified_comments: vec![weak_evidence_comment()],
        escalation_candidates: vec![escalation_candidate(EscalationLabel::MoveToAdr)],
    };

    let draft = build_draft_reply(&gate, 3);

    assert_eq!(draft.reply_type, ReplyType::MoveToAdr);
    assert!(draft.body.contains("moving the design decision to an ADR"));
}

#[test]
fn draft_reply_preserves_rfc_and_human_judgment_routes() {
    let mut rfc_gate = empty_gate();
    rfc_gate.escalation_candidates = vec![escalation_candidate(EscalationLabel::MoveToRfc)];

    let rfc_draft = build_draft_reply(&rfc_gate, 3);
    assert_eq!(rfc_draft.reply_type, ReplyType::MoveToRfc);
    assert!(rfc_draft.body.contains("moving it to an RFC"));

    let mut human_gate = empty_gate();
    human_gate.escalation_candidates =
        vec![escalation_candidate(EscalationLabel::NeedsHumanJudgment)];

    let human_draft = build_draft_reply(&human_gate, 3);
    assert_eq!(human_draft.reply_type, ReplyType::NeedsHumanJudgment);
    assert!(human_draft.body.contains("human judgment call"));
}

#[test]
fn draft_reply_asks_for_evidence_when_changes_requested_has_no_residual_blocker() {
    let mut gate = empty_gate();
    gate.review_decision_summary =
        ReviewDecisionSummary::from_states(&[String::from("CHANGES_REQUESTED")]);

    let draft = build_draft_reply(&gate, 3);

    assert_eq!(draft.reply_type, ReplyType::AskForEvidence);
    assert!(draft.target_comment_id.is_none());
    assert!(draft.body.contains("changes requested"));
}

#[test]
fn report_contains_required_sections() {
    let gate = GateArtifact {
        status: Status::Ok,
        data_coverage: DataCoverage::Full,
        review_signal: ReviewSignal::Blocked,
        reason: None,
        comments_analyzed: 1,
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
            author: String::from("reviewer-a"),
        }],
        counts: GateCounts::default(),
        candidate_blockers: Vec::new(),
        downgraded_comments: Vec::new(),
        duplicates_collapsed: Vec::new(),
        warnings: Vec::new(),
        config_snapshot: GateConfigSnapshot::default(),
        review_decision_summary: None,
        classified_comments: Vec::new(),
        escalation_candidates: Vec::new(),
    };
    let draft = DraftReplyArtifact {
        status: Status::Ok,
        data_coverage: DataCoverage::Full,
        review_signal: ReviewSignal::Blocked,
        reason: None,
        reply_type: ReplyType::Accept,
        target_comment_id: Some(String::from("12")),
        body: String::from("Thanks.\nI will address this here."),
    };

    let report = build_report_markdown(
        ReportHeader {
            run_status: Status::Ok,
            data_coverage: DataCoverage::Full,
            review_signal: ReviewSignal::Blocked,
            residual_blockers: 1,
        },
        None,
        ReportInputs {
            scan: Some(&ScanArtifact {
                status: Status::Ok,
                data_coverage: DataCoverage::Full,
                review_signal: ReviewSignal::Unknown,
                reason: None,
                scan_partial: false,
                repo_root: Some(String::from("/tmp/review-firewall")),
                branch: Some(String::from("feature/test")),
                pr: Default::default(),
                files_changed: 1,
                review_comments: 1,
                threads: 1,
                codeowners_found: false,
                policy_found: false,
                product_boundary: Default::default(),
                changed_files: Vec::new(),
                comments: Vec::new(),
                issue_comments: Vec::new(),
                review_threads: Vec::new(),
                partial_sources: Vec::new(),
                warnings: Vec::new(),
            }),
            source_coverage: Some(&full_source_coverage()),
            gate: Some(&gate),
            draft_reply: Some(&draft),
            escalation_markdown: Some("# Escalation\n\nNo ADR/RFC candidates were found."),
            ..Default::default()
        },
    );

    assert!(report.contains("## Residual blockers"));
    assert!(report.contains("## PM summary"));
    assert!(report.contains("## Author action list"));
    assert!(report.contains("## Source coverage"));
    assert!(report.contains("RUN_STATUS: OK"));
    assert!(report.contains("DATA_COVERAGE: FULL"));
    assert!(report.contains("REVIEW_SIGNAL: BLOCKED"));
    assert!(report.contains("RESIDUAL_BLOCKERS: 1"));
    assert!(report.contains("Review-input coverage: FULL"));
    assert!(report.contains("- PR metadata: FULL (required, 1 seen)"));
}

#[test]
fn report_counts_non_adr_escalation_candidates() {
    let report = build_report_markdown(
        ReportHeader {
            run_status: Status::Ok,
            data_coverage: DataCoverage::Full,
            review_signal: ReviewSignal::Clear,
            residual_blockers: 0,
        },
        None,
        ReportInputs {
            escalation_markdown: Some("# Escalation\n\n# RFC Candidate\n\n## Title\nAPI policy"),
            ..Default::default()
        },
    );

    assert!(report.contains("Action: move the long-running design thread to ADR/RFC"));
    assert!(report.contains("Move the long-running design thread into ADR/RFC"));
}

#[test]
fn report_avoids_no_blocker_claim_when_analysis_is_partial() {
    let gate = non_authoritative_gate(Status::Partial, "CODEOWNERS could not be read");

    let report = build_report_markdown(
        ReportHeader {
            run_status: Status::Partial,
            data_coverage: DataCoverage::Partial,
            review_signal: ReviewSignal::Unknown,
            residual_blockers: 0,
        },
        Some("CODEOWNERS could not be read"),
        ReportInputs {
            source_coverage: Some(&partial_source_coverage()),
            gate: Some(&gate),
            escalation_markdown: Some("# Escalation\n\nNo ADR/RFC candidates were found."),
            ..Default::default()
        },
    );

    assert!(report.contains("- unknown: blocker analysis did not complete"));
    assert!(report.contains("no merge-safety claim is available"));
    assert!(report.contains("inspect source coverage"));
    assert!(report.contains("Review-input coverage: PARTIAL"));
    assert!(report.contains("Failure reason: gh_not_authenticated"));
    assert!(!report.contains("no current merge blocker was extracted"));
    assert!(!report.contains("continue normal PR follow-up"));
}

#[test]
fn report_surfaces_changes_requested_as_informational_context() {
    let mut gate = empty_gate();
    gate.review_decision_summary =
        ReviewDecisionSummary::from_states(&[String::from("CHANGES_REQUESTED")]);

    let report = build_report_markdown(
        ReportHeader {
            run_status: Status::Ok,
            data_coverage: DataCoverage::Full,
            review_signal: ReviewSignal::Clear,
            residual_blockers: 0,
        },
        None,
        ReportInputs {
            source_coverage: Some(&full_source_coverage()),
            gate: Some(&gate),
            escalation_markdown: Some("# Escalation\n\nNo ADR/RFC candidates were found."),
            ..Default::default()
        },
    );

    assert!(report.contains("REVIEW_DECISIONS: CHANGES_REQUESTED (informational only)"));
    assert!(report.contains("GitHub still shows changes requested"));
    assert!(report.contains("Align the remaining CHANGES_REQUESTED state"));
}

#[test]
fn report_names_missing_source_coverage_artifact_without_hiding_other_sections() {
    let report = build_report_markdown(
        ReportHeader {
            run_status: Status::Partial,
            data_coverage: DataCoverage::Partial,
            review_signal: ReviewSignal::Unknown,
            residual_blockers: 0,
        },
        Some("source coverage missing"),
        ReportInputs {
            scan: Some(&ScanArtifact {
                status: Status::Partial,
                data_coverage: DataCoverage::Partial,
                review_signal: ReviewSignal::Unknown,
                reason: Some(String::from("source coverage missing")),
                scan_partial: true,
                repo_root: Some(String::from("/tmp/review-firewall")),
                branch: Some(String::from("feature/test")),
                pr: Default::default(),
                files_changed: 0,
                review_comments: 0,
                threads: 0,
                codeowners_found: false,
                policy_found: false,
                product_boundary: Default::default(),
                changed_files: Vec::new(),
                comments: Vec::new(),
                issue_comments: Vec::new(),
                review_threads: Vec::new(),
                partial_sources: vec![String::from("pr_metadata"), String::from("review_comments")],
                warnings: Vec::new(),
            }),
            source_coverage_notice: Some(
                "source_coverage.json not found; run review-firewall scan first",
            ),
            gate: Some(&non_authoritative_gate(
                Status::Partial,
                "review comments were partially unavailable",
            )),
            escalation_markdown: Some("# Escalation\n\nNo ADR/RFC candidates were found."),
            ..Default::default()
        },
    );

    assert!(report.contains("## Source coverage"));
    assert!(report.contains("Incomplete required sources: 2"));
    assert!(report.contains("source_coverage.json not found; run review-firewall scan first"));
}

fn non_authoritative_gate(status: Status, reason: &str) -> GateArtifact {
    GateArtifact {
        status,
        data_coverage: DataCoverage::Partial,
        review_signal: ReviewSignal::Unknown,
        reason: Some(String::from(reason)),
        comments_analyzed: 0,
        residual_blockers: Vec::new(),
        counts: GateCounts::default(),
        candidate_blockers: Vec::new(),
        downgraded_comments: Vec::new(),
        duplicates_collapsed: Vec::new(),
        warnings: Vec::new(),
        config_snapshot: GateConfigSnapshot::default(),
        review_decision_summary: None,
        classified_comments: Vec::new(),
        escalation_candidates: Vec::new(),
    }
}

fn empty_gate() -> GateArtifact {
    GateArtifact {
        status: Status::Ok,
        data_coverage: DataCoverage::Full,
        review_signal: ReviewSignal::Clear,
        reason: None,
        comments_analyzed: 0,
        residual_blockers: Vec::new(),
        counts: GateCounts::default(),
        candidate_blockers: Vec::new(),
        downgraded_comments: Vec::new(),
        duplicates_collapsed: Vec::new(),
        warnings: Vec::new(),
        config_snapshot: GateConfigSnapshot::default(),
        review_decision_summary: None,
        classified_comments: Vec::new(),
        escalation_candidates: Vec::new(),
    }
}

fn full_source_coverage() -> SourceCoverageArtifact {
    SourceCoverageArtifact {
        status: Status::Ok,
        data_coverage: DataCoverage::Full,
        review_signal: ReviewSignal::Unknown,
        reason: None,
        sources: vec![
            SourceCoverageEntry::new(
                SourceCoverageName::PrMetadata,
                true,
                SourceCoverageStatus::Full,
                1,
                None,
                None,
            ),
            SourceCoverageEntry::new(
                SourceCoverageName::ChangedFiles,
                true,
                SourceCoverageStatus::Full,
                3,
                None,
                None,
            ),
            SourceCoverageEntry::new(
                SourceCoverageName::ReviewComments,
                true,
                SourceCoverageStatus::Full,
                8,
                None,
                None,
            ),
            SourceCoverageEntry::new(
                SourceCoverageName::IssueComments,
                true,
                SourceCoverageStatus::Full,
                2,
                None,
                None,
            ),
        ],
        warnings: Vec::new(),
    }
}

fn partial_source_coverage() -> SourceCoverageArtifact {
    SourceCoverageArtifact {
        status: Status::Partial,
        data_coverage: DataCoverage::Partial,
        review_signal: ReviewSignal::Unknown,
        reason: Some(String::from("review comments were partially unavailable")),
        sources: vec![
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
                42,
                Some(SourceFailureReason::GhNotAuthenticated),
                Some(String::from(
                    "GitHub CLI could not authenticate while fetching review comments.",
                )),
            ),
        ],
        warnings: Vec::new(),
    }
}

fn weak_evidence_comment() -> ClassifiedComment {
    ClassifiedComment {
        comment: CommentRecord {
            comment_id: String::from("weak-1"),
            thread_id: String::from("weak-1"),
            author: String::from("reviewer-a"),
            body: String::from("This feels risky for the API boundary."),
            path: Some(String::from("src/api/response.rs")),
            source: CommentSource::ReviewComment,
            reply_to_comment_id: None,
            created_at: None,
            line: Some(10),
            original_line: Some(10),
        },
        comment_type: CommentType::Suggestion,
        concern: Some(BlockerConcern::Api),
        failure_mode: None,
        evidence_class: Some(EvidenceClass::KeywordOnly),
        evidence: Vec::new(),
        present_pr_impact: false,
        owner_match: true,
        ownership_scope: rf_core::domain::OwnershipScope::Exact,
        advisory_weight: AdvisoryWeight::High,
        duplicate_of_comment_id: None,
    }
}

fn escalation_candidate(label: EscalationLabel) -> EscalationCandidate {
    EscalationCandidate {
        thread_id: String::from("thread-1"),
        root_comment_id: String::from("root-1"),
        label,
        title: String::from("Contract discussion in response.rs"),
        path: Some(String::from("src/api/response.rs")),
        roundtrips: 4,
        participants: vec![String::from("reviewer-a"), String::from("author")],
        position_a: String::from("Keep the current schema in this PR."),
        position_b: String::from("Move the contract change to follow-up design work."),
        why: String::from("The thread exceeded the PR-local roundtrip budget."),
    }
}
