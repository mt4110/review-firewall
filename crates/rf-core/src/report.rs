use crate::domain::{DraftReplyArtifact, GateArtifact, ScanArtifact, Status};

pub fn build_report_markdown(
    status: Status,
    reason: Option<&str>,
    scan: Option<&ScanArtifact>,
    gate: Option<&GateArtifact>,
    draft_reply: Option<&DraftReplyArtifact>,
    escalation_markdown: Option<&str>,
) -> String {
    let mut lines = vec![format!("STATUS: {}", status.terminal_label())];
    if let Some(reason) = reason
        && !reason.is_empty()
    {
        lines.push(format!("REASON: {reason}"));
    }
    lines.push(String::new());
    lines.push(String::from("# Review Firewall Report"));
    lines.push(String::new());
    lines.push(String::from("## Residual blockers"));

    match gate {
        Some(gate) if !gate.residual_blockers.is_empty() => {
            for blocker in &gate.residual_blockers {
                lines.push(format!(
                    "- {}: {}",
                    concern_label(&blocker.concern),
                    blocker.failure_mode
                ));
            }
        }
        _ => lines.push(String::from("- none")),
    }

    lines.push(String::new());
    lines.push(String::from("## PM summary"));
    let residual_count = gate
        .map(|artifact| artifact.residual_blockers.len())
        .unwrap_or(0);
    lines.push(format!("Residual blockers: {residual_count}"));
    if let Some(first_blocker) = gate.and_then(|artifact| artifact.residual_blockers.first()) {
        lines.push(format!("Impact: {}", first_blocker.failure_mode));
        lines.push(String::from(
            "Action: decide whether to fix in this PR or move the broader design issue out of band",
        ));
    } else {
        let escalation_count = escalation_markdown
            .map(|markdown| markdown.matches("# ADR Candidate").count())
            .unwrap_or(0);
        lines.push(String::from(
            "Impact: no current merge blocker was extracted",
        ));
        lines.push(if escalation_count > 0 {
            String::from("Action: move the long-running design thread to ADR/RFC")
        } else {
            String::from("Action: continue normal PR follow-up")
        });
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
    if escalation_markdown
        .map(|markdown| markdown.contains("# ADR Candidate"))
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

fn reply_label(reply_type: &crate::domain::ReplyType) -> &'static str {
    match reply_type {
        crate::domain::ReplyType::Accept => "accept",
        crate::domain::ReplyType::Decline => "decline",
        crate::domain::ReplyType::Move => "move",
    }
}
