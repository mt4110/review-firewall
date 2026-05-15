use crate::domain::{
    DataCoverage, DraftReplyArtifact, GateArtifact, ReviewSignal, ScanArtifact,
    SourceCoverageArtifact, SourceCoverageEntry, SourceCoverageName, SourceCoverageStatus, Status,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReportHeader {
    pub run_status: Status,
    pub data_coverage: DataCoverage,
    pub review_signal: ReviewSignal,
    pub residual_blockers: usize,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ReportInputs<'a> {
    pub scan: Option<&'a ScanArtifact>,
    pub source_coverage: Option<&'a SourceCoverageArtifact>,
    pub source_coverage_notice: Option<&'a str>,
    pub gate: Option<&'a GateArtifact>,
    pub draft_reply: Option<&'a DraftReplyArtifact>,
    pub escalation_markdown: Option<&'a str>,
}

pub fn build_report_markdown(
    header: ReportHeader,
    reason: Option<&str>,
    inputs: ReportInputs<'_>,
) -> String {
    let mut lines = vec![
        format!("RUN_STATUS: {}", header.run_status.terminal_label()),
        format!("DATA_COVERAGE: {}", header.data_coverage.terminal_label()),
        format!("REVIEW_SIGNAL: {}", header.review_signal.terminal_label()),
        format!("RESIDUAL_BLOCKERS: {}", header.residual_blockers),
        format!("STATUS: {}", header.run_status.terminal_label()),
    ];
    if let Some(reason) = reason
        && !reason.is_empty()
    {
        lines.push(format!("REASON: {reason}"));
    }
    if let Some(summary) = inputs
        .gate
        .and_then(|artifact| artifact.review_decision_summary.as_ref())
    {
        lines.push(format!(
            "REVIEW_DECISIONS: {} (informational only)",
            summary.states.join(", ")
        ));
    }
    lines.push(String::new());
    lines.push(String::from("# Review Firewall Report"));
    lines.push(String::new());
    lines.push(String::from("## Residual blockers"));

    match inputs.gate {
        Some(gate) if !gate.residual_blockers.is_empty() => {
            for blocker in &gate.residual_blockers {
                lines.push(format!(
                    "- {} [{}]: {}",
                    concern_label(&blocker.concern),
                    evidence_class_label(&blocker.evidence_class),
                    blocker.failure_mode
                ));
            }
        }
        _ if header.review_signal == ReviewSignal::Unknown => {
            lines.push(String::from("- unknown: blocker analysis did not complete"))
        }
        _ => lines.push(String::from("- none")),
    }

    lines.push(String::new());
    lines.push(String::from("## PM summary"));
    lines.push(format!("Residual blockers: {}", header.residual_blockers));
    if let Some(summary) = inputs
        .gate
        .and_then(|artifact| artifact.review_decision_summary.as_ref())
    {
        lines.push(format!(
            "Reviewer state: {} (informational only)",
            summary.states.join(", ")
        ));
    }
    if let Some(first_blocker) = inputs
        .gate
        .and_then(|artifact| artifact.residual_blockers.first())
    {
        lines.push(format!("Impact: {}", first_blocker.failure_mode));
        lines.push(String::from(
            "Action: decide whether to fix in this PR or move the broader design issue out of band",
        ));
    } else {
        let escalation_count = inputs
            .escalation_markdown
            .map(count_escalation_candidates)
            .unwrap_or(0);
        if header.review_signal == ReviewSignal::Unknown {
            lines.push(String::from(
                "Impact: blocker analysis did not complete; no merge-safety claim is available",
            ));
            lines.push(String::from(
                "Action: inspect source coverage and resolve missing review inputs before making a merge decision",
            ));
        } else if inputs
            .gate
            .and_then(|artifact| artifact.review_decision_summary.as_ref())
            .is_some_and(|summary| summary.changes_requested)
        {
            lines.push(String::from(
                "Impact: no residual blocker was extracted, but GitHub still shows changes requested.",
            ));
            lines.push(String::from(
                "Action: align the remaining review state with a concrete PR-local blocker or clear the stale request",
            ));
        } else {
            lines.push(String::from(
                "Impact: no current merge blocker was extracted",
            ));
            lines.push(if escalation_count > 0 {
                String::from("Action: move the long-running design thread to ADR/RFC")
            } else {
                String::from("Action: continue normal PR follow-up")
            });
        }
    }

    lines.push(String::new());
    lines.push(String::from("## Author action list"));
    let mut action_index = 1usize;
    if let Some(gate) = inputs.gate {
        for blocker in gate.residual_blockers.iter().take(3) {
            lines.push(format!(
                "{action_index}. Address blocker #{}: {}",
                blocker.comment_id, blocker.failure_mode
            ));
            action_index += 1;
        }
    }
    if let Some(draft_reply) = inputs.draft_reply {
        lines.push(format!(
            "{action_index}. Use the {} reply draft: {}",
            reply_label(&draft_reply.reply_type),
            draft_reply.body.replace('\n', " / ")
        ));
        action_index += 1;
    }
    if header.review_signal == ReviewSignal::Clear
        && inputs
            .gate
            .and_then(|artifact| artifact.review_decision_summary.as_ref())
            .is_some_and(|summary| summary.changes_requested)
    {
        lines.push(format!(
            "{action_index}. Align the remaining CHANGES_REQUESTED state with a concrete PR-local blocker or dismiss it as stale"
        ));
        action_index += 1;
    }
    if inputs
        .escalation_markdown
        .map(|markdown| count_escalation_candidates(markdown) > 0)
        .unwrap_or(false)
    {
        lines.push(format!(
            "{action_index}. Move the long-running design thread into ADR/RFC before expanding PR scope"
        ));
    } else if action_index == 1 {
        lines.push(String::from(
            "1. No immediate action is required beyond normal PR follow-up",
        ));
    }

    if let Some(scan) = inputs.scan
        && scan.pr.number.is_none()
        && scan.pr.title.is_empty()
    {
        lines.push(String::new());
        lines.push(String::from("<!-- scan metadata was partial -->"));
    }

    lines.push(String::new());
    lines.push(String::from("## Source coverage"));
    append_source_coverage(
        &mut lines,
        header.data_coverage,
        inputs.source_coverage,
        inputs.source_coverage_notice,
        inputs.scan,
    );

    lines.join("\n")
}

fn append_source_coverage(
    lines: &mut Vec<String>,
    fallback_data_coverage: DataCoverage,
    source_coverage: Option<&SourceCoverageArtifact>,
    source_coverage_notice: Option<&str>,
    scan: Option<&ScanArtifact>,
) {
    if let Some(source_coverage) = source_coverage {
        let incomplete_required = source_coverage
            .sources
            .iter()
            .filter(|source| source.required && source.status != SourceCoverageStatus::Full)
            .count();
        lines.push(format!(
            "Review-input coverage: {}",
            source_coverage.data_coverage.terminal_label()
        ));
        lines.push(format!(
            "Incomplete required sources: {incomplete_required}"
        ));
        if let Some(reason) = source_coverage.reason.as_deref()
            && !reason.is_empty()
        {
            lines.push(format!("Coverage reason: {reason}"));
        }
        if source_coverage.sources.is_empty() {
            lines.push(String::from("- none"));
            return;
        }
        for source in &source_coverage.sources {
            lines.push(format_source_coverage_entry(source));
            if let Some(failure_reason) = source.failure_reason {
                lines.push(format!("  Failure reason: {}", failure_reason.as_str()));
            }
            if let Some(detail) = source.detail.as_deref()
                && !detail.is_empty()
            {
                lines.push(format!("  Detail: {detail}"));
            }
            if let Some(retry_hint) = source.retry_hint.as_deref()
                && !retry_hint.is_empty()
            {
                lines.push(format!("  Next: {retry_hint}"));
            }
        }
        return;
    }

    lines.push(format!(
        "Review-input coverage: {}",
        fallback_data_coverage.terminal_label()
    ));
    if let Some(scan) = scan {
        lines.push(format!(
            "Incomplete required sources: {}",
            scan.partial_sources.len()
        ));
    }
    if let Some(notice) = source_coverage_notice
        && !notice.is_empty()
    {
        lines.push(format!("- unavailable: {notice}"));
    } else {
        lines.push(String::from(
            "- unavailable: source_coverage.json was not available for this run",
        ));
    }
}

fn format_source_coverage_entry(source: &SourceCoverageEntry) -> String {
    let requirement = if source.required {
        "required"
    } else {
        "optional"
    };
    format!(
        "- {}: {} ({requirement}, {} seen)",
        source_coverage_label(source.name),
        source_coverage_status_label(source.status),
        source.items_seen
    )
}

fn concern_label(concern: &crate::domain::BlockerConcern) -> &'static str {
    match concern {
        crate::domain::BlockerConcern::Correctness => "correctness",
        crate::domain::BlockerConcern::Security => "security",
        crate::domain::BlockerConcern::Performance => "performance",
        crate::domain::BlockerConcern::Operability => "operability",
        crate::domain::BlockerConcern::Api => "api",
    }
}

fn count_escalation_candidates(markdown: &str) -> usize {
    [
        "# ADR Candidate",
        "# RFC Candidate",
        "# Human Judgment Candidate",
    ]
    .iter()
    .map(|header| markdown.matches(header).count())
    .sum()
}

fn reply_label(reply_type: &crate::domain::ReplyType) -> &'static str {
    match reply_type {
        crate::domain::ReplyType::Accept => "accept",
        crate::domain::ReplyType::AskForEvidence => "ask_for_evidence",
        crate::domain::ReplyType::AskForScope => "ask_for_scope",
        crate::domain::ReplyType::MoveToAdr => "move_to_adr",
        crate::domain::ReplyType::MoveToRfc => "move_to_rfc",
        crate::domain::ReplyType::NeedsHumanJudgment => "needs_human_judgment",
        crate::domain::ReplyType::CannotClassify => "cannot_classify",
    }
}

fn evidence_class_label(class: &crate::domain::EvidenceClass) -> &'static str {
    match class {
        crate::domain::EvidenceClass::CausalRuntimeFailure => "causal_runtime_failure",
        crate::domain::EvidenceClass::ContractDelta => "contract_delta",
        crate::domain::EvidenceClass::ReproCondition => "repro_condition",
        crate::domain::EvidenceClass::SecurityCondition => "security_condition",
        crate::domain::EvidenceClass::CiTestFailure => "ci_test_failure",
        crate::domain::EvidenceClass::ConcreteReference => "concrete_reference",
        crate::domain::EvidenceClass::KeywordOnly => "keyword_only",
        crate::domain::EvidenceClass::PathOnly => "path_only",
        crate::domain::EvidenceClass::NoiseOnly => "noise_only",
    }
}

fn source_coverage_label(name: SourceCoverageName) -> &'static str {
    match name {
        SourceCoverageName::RepoRoot => "Repo root",
        SourceCoverageName::CurrentBranch => "Current branch",
        SourceCoverageName::Config => "Config",
        SourceCoverageName::Codeowners => "CODEOWNERS",
        SourceCoverageName::PrMetadata => "PR metadata",
        SourceCoverageName::ChangedFiles => "Changed files",
        SourceCoverageName::ReviewComments => "Review comments",
        SourceCoverageName::ReviewBodyComments => "Review body comments",
        SourceCoverageName::IssueComments => "Issue comments",
        SourceCoverageName::ReviewDecision => "Review decision",
    }
}

fn source_coverage_status_label(status: SourceCoverageStatus) -> &'static str {
    match status {
        SourceCoverageStatus::Full => "FULL",
        SourceCoverageStatus::Partial => "PARTIAL",
        SourceCoverageStatus::Failed => "FAILED",
        SourceCoverageStatus::Skipped => "SKIPPED",
    }
}
