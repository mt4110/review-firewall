use rf_core::build_draft_reply;
use rf_core::build_report_markdown;
use rf_core::domain::{
    AdvisoryWeight, BlockerConcern, DraftReplyArtifact, GateArtifact, GateConfigSnapshot,
    GateCounts, ReplyType, ResidualBlocker, ScanArtifact, Status,
};

#[test]
fn draft_reply_respects_max_lines() {
    let gate = GateArtifact {
        status: Status::Ok,
        reason: None,
        comments_analyzed: 1,
        residual_blockers: vec![ResidualBlocker {
            comment_id: String::from("12"),
            concern: BlockerConcern::Correctness,
            failure_mode: String::from("partial status may break response contract"),
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
    assert_eq!(draft.reply_type, ReplyType::Decline);
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
    assert_eq!(draft.reply_type, ReplyType::Decline);
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
fn report_contains_required_sections() {
    let gate = GateArtifact {
        status: Status::Ok,
        reason: None,
        comments_analyzed: 1,
        residual_blockers: vec![ResidualBlocker {
            comment_id: String::from("12"),
            concern: BlockerConcern::Correctness,
            failure_mode: String::from("partial status may break response contract"),
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
        classified_comments: Vec::new(),
        escalation_candidates: Vec::new(),
    };
    let draft = DraftReplyArtifact {
        status: Status::Ok,
        reason: None,
        reply_type: ReplyType::Accept,
        target_comment_id: Some(String::from("12")),
        body: String::from("Thanks.\nI will address this here."),
    };

    let report = build_report_markdown(
        Status::Ok,
        None,
        Some(&ScanArtifact {
            status: Status::Ok,
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
        Some(&gate),
        Some(&draft),
        Some("# Escalation\n\nNo ADR/RFC candidates were found."),
    );

    assert!(report.contains("## Residual blockers"));
    assert!(report.contains("## PM summary"));
    assert!(report.contains("## Author action list"));
}

#[test]
fn report_counts_non_adr_escalation_candidates() {
    let report = build_report_markdown(
        Status::Ok,
        None,
        None,
        None,
        None,
        Some("# Escalation\n\n# RFC Candidate\n\n## Title\nAPI policy"),
    );

    assert!(report.contains("Action: move the long-running design thread to ADR/RFC"));
    assert!(report.contains("Move the long-running design thread into ADR/RFC"));
}

#[test]
fn report_avoids_no_blocker_claim_when_analysis_is_partial() {
    let gate = non_authoritative_gate(Status::Partial, "CODEOWNERS could not be read");

    let report = build_report_markdown(
        Status::Partial,
        Some("CODEOWNERS could not be read"),
        None,
        Some(&gate),
        None,
        Some("# Escalation\n\nNo ADR/RFC candidates were found."),
    );

    assert!(report.contains("- unknown: blocker analysis did not complete"));
    assert!(report.contains("no merge-safety claim is available"));
    assert!(!report.contains("no current merge blocker was extracted"));
    assert!(!report.contains("continue normal PR follow-up"));
}

fn non_authoritative_gate(status: Status, reason: &str) -> GateArtifact {
    GateArtifact {
        status,
        reason: Some(String::from(reason)),
        comments_analyzed: 0,
        residual_blockers: Vec::new(),
        counts: GateCounts::default(),
        candidate_blockers: Vec::new(),
        downgraded_comments: Vec::new(),
        duplicates_collapsed: Vec::new(),
        warnings: Vec::new(),
        config_snapshot: GateConfigSnapshot::default(),
        classified_comments: Vec::new(),
        escalation_candidates: Vec::new(),
    }
}
