use crate::domain::{DraftReplyArtifact, EscalationLabel, GateArtifact, ReplyType, Status};

pub fn build_draft_reply(gate: &GateArtifact, max_lines: usize) -> DraftReplyArtifact {
    let max_lines = max_lines.max(1);

    if gate.status != Status::Ok || gate.review_signal == crate::domain::ReviewSignal::Unknown {
        return DraftReplyArtifact {
            status: gate.status,
            data_coverage: gate.data_coverage,
            review_signal: gate.review_signal,
            reason: gate.reason.clone(),
            reply_type: ReplyType::CannotClassify,
            target_comment_id: None,
            body: limit_lines(
                &format!(
                    "I could not complete blocker analysis for this PR, so I cannot draft a safe review reply yet.\nReason: {}\nRerun review-firewall scan and gate before posting a review response.",
                    gate.reason
                        .as_deref()
                        .unwrap_or("gate analysis did not complete successfully")
                ),
                max_lines,
            ),
        };
    }

    if let Some(blocker) = gate.residual_blockers.first() {
        let action = blocker
            .path
            .as_deref()
            .map(|path| format!("updating {path}"))
            .unwrap_or_else(|| {
                format!(
                    "tightening the {} handling",
                    concern_label(&blocker.concern)
                )
            });

        return DraftReplyArtifact {
            status: gate.status,
            data_coverage: gate.data_coverage,
            review_signal: gate.review_signal,
            reason: gate.reason.clone(),
            reply_type: ReplyType::Accept,
            target_comment_id: Some(blocker.comment_id.clone()),
            body: limit_lines(
                &format!(
                    "Thanks. I agree this is a {} issue in this PR.\nI will address it here by {}.",
                    concern_label(&blocker.concern),
                    action
                ),
                max_lines,
            ),
        };
    }

    if let Some(candidate) = gate
        .escalation_candidates
        .iter()
        .find(|candidate| candidate.label != EscalationLabel::StayInPr)
    {
        return DraftReplyArtifact {
            status: gate.status,
            data_coverage: gate.data_coverage,
            review_signal: gate.review_signal,
            reason: gate.reason.clone(),
            reply_type: escalation_reply_type(candidate.label),
            target_comment_id: Some(candidate.root_comment_id.clone()),
            body: limit_lines(escalation_reply_body(candidate.label), max_lines),
        };
    }

    if let Some(comment) = gate.classified_comments.iter().find(|comment| {
        comment.duplicate_of_comment_id.is_none()
            && comment
                .evidence_class
                .is_some_and(|class| !class.supports_residual_blocker())
            && comment.concern.is_some()
    }) {
        return DraftReplyArtifact {
            status: gate.status,
            data_coverage: gate.data_coverage,
            review_signal: gate.review_signal,
            reason: gate.reason.clone(),
            reply_type: ReplyType::AskForEvidence,
            target_comment_id: Some(comment.comment.comment_id.clone()),
            body: limit_lines(
                "Thanks. I want to make sure I address the real PR-local risk here.\nCould you point me to the concrete failure mode, repro condition, or contract change you see in this diff?",
                max_lines,
            ),
        };
    }

    if gate.review_signal == crate::domain::ReviewSignal::Clear
        && gate
            .review_decision_summary
            .as_ref()
            .is_some_and(|summary| summary.changes_requested)
    {
        return DraftReplyArtifact {
            status: gate.status,
            data_coverage: gate.data_coverage,
            review_signal: gate.review_signal,
            reason: gate.reason.clone(),
            reply_type: ReplyType::AskForEvidence,
            target_comment_id: None,
            body: limit_lines(
                "Thanks. I do not see a concrete PR-local blocker in the current diff yet.\nCould you point me to the specific failure mode, repro condition, or contract delta that should keep this review in changes requested?",
                max_lines,
            ),
        };
    }

    DraftReplyArtifact {
        status: gate.status,
        data_coverage: gate.data_coverage,
        review_signal: gate.review_signal,
        reason: gate.reason.clone(),
        reply_type: ReplyType::AskForScope,
        target_comment_id: gate
            .classified_comments
            .first()
            .map(|comment| comment.comment.comment_id.clone()),
        body: limit_lines(
            "Thanks. I want to keep this PR focused on concrete PR-local impact.\nIf this concern is broader than the current change, I can move it into follow-up design work.",
            max_lines,
        ),
    }
}

fn escalation_reply_type(label: EscalationLabel) -> ReplyType {
    match label {
        EscalationLabel::MoveToAdr => ReplyType::MoveToAdr,
        EscalationLabel::MoveToRfc => ReplyType::MoveToRfc,
        EscalationLabel::NeedsHumanJudgment => ReplyType::NeedsHumanJudgment,
        EscalationLabel::StayInPr => ReplyType::AskForScope,
    }
}

fn escalation_reply_body(label: EscalationLabel) -> &'static str {
    match label {
        EscalationLabel::MoveToAdr => {
            "Thanks. This looks broader than a PR-local blocker.\nI propose moving the design decision to an ADR and keeping this PR scoped to the agreed behavior."
        }
        EscalationLabel::MoveToRfc => {
            "Thanks. This looks like a cross-boundary contract decision.\nI propose moving it to an RFC and keeping this PR scoped to the current agreed interface."
        }
        EscalationLabel::NeedsHumanJudgment => {
            "Thanks. This thread looks stuck beyond safe PR iteration.\nI propose getting a human judgment call and keeping this PR scoped until that decision is made."
        }
        EscalationLabel::StayInPr => {
            "Thanks. I want to keep this PR focused on concrete PR-local impact.\nIf this concern is broader than the current change, I can move it into follow-up design work."
        }
    }
}

fn limit_lines(body: &str, max_lines: usize) -> String {
    body.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(max_lines)
        .collect::<Vec<_>>()
        .join("\n")
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
