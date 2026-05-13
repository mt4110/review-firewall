use crate::domain::{
    DataCoverage, DraftReplyArtifact, GateArtifact, ReviewSignal, ScanArtifact, Status,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReportHeader {
    pub run_status: Status,
    pub data_coverage: DataCoverage,
    pub review_signal: ReviewSignal,
    pub residual_blockers: usize,
}

pub fn build_report_markdown(
    header: ReportHeader,
    reason: Option<&str>,
    scan: Option<&ScanArtifact>,
    gate: Option<&GateArtifact>,
    draft_reply: Option<&DraftReplyArtifact>,
    escalation_markdown: Option<&str>,
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
    if let Some(summary) = gate.and_then(|artifact| artifact.review_decision_summary.as_ref()) {
        lines.push(format!(
            "REVIEW_DECISIONS: {} (informational only)",
            summary.states.join(", ")
        ));
    }
    lines.push(String::new());
    lines.push(String::from("# Review Firewall Report"));
    lines.push(String::new());
    lines.push(String::from("## Residual blockers"));

    match gate {
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
    if let Some(summary) = gate.and_then(|artifact| artifact.review_decision_summary.as_ref()) {
        lines.push(format!(
            "Reviewer state: {} (informational only)",
            summary.states.join(", ")
        ));
    }
    if let Some(first_blocker) = gate.and_then(|artifact| artifact.residual_blockers.first()) {
        lines.push(format!("Impact: {}", first_blocker.failure_mode));
        lines.push(String::from(
            "Action: decide whether to fix in this PR or move the broader design issue out of band",
        ));
    } else {
        let escalation_count = escalation_markdown
            .map(count_escalation_candidates)
            .unwrap_or(0);
        if header.review_signal == ReviewSignal::Unknown {
            lines.push(String::from(
                "Impact: blocker analysis did not complete; no merge-safety claim is available",
            ));
            lines.push(String::from(
                "Action: resolve missing analysis inputs before making a merge decision",
            ));
        } else if gate
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
    if let Some(gate) = gate {
        for blocker in gate.residual_blockers.iter().take(3) {
            lines.push(format!(
                "{action_index}. Address blocker #{}: {}",
                blocker.comment_id, blocker.failure_mode
            ));
            action_index += 1;
        }
    }
    if let Some(draft_reply) = draft_reply {
        lines.push(format!(
            "{action_index}. Use the {} reply draft: {}",
            reply_label(&draft_reply.reply_type),
            draft_reply.body.replace('\n', " / ")
        ));
        action_index += 1;
    }
    if header.review_signal == ReviewSignal::Clear
        && gate
            .and_then(|artifact| artifact.review_decision_summary.as_ref())
            .is_some_and(|summary| summary.changes_requested)
    {
        lines.push(format!(
            "{action_index}. Align the remaining CHANGES_REQUESTED state with a concrete PR-local blocker or dismiss it as stale"
        ));
        action_index += 1;
    }
    if escalation_markdown
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

    if let Some(scan) = scan
        && scan.pr.number.is_none()
        && scan.pr.title.is_empty()
    {
        lines.push(String::new());
        lines.push(String::from("<!-- scan metadata was partial -->"));
    }

    lines.join("\n")
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
