use crate::domain::{DraftReplyArtifact, GateArtifact, ReplyType, Status};

pub fn build_draft_reply(gate: &GateArtifact, max_lines: usize) -> DraftReplyArtifact {
    let max_lines = max_lines.max(1);

    if gate.status == Status::Error {
        return DraftReplyArtifact {
            status: gate.status,
            reason: gate.reason.clone(),
            reply_type: ReplyType::Decline,
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
        .find(|candidate| !matches!(candidate.label, crate::domain::EscalationLabel::StayInPr))
    {
        return DraftReplyArtifact {
            status: gate.status,
            reason: gate.reason.clone(),
            reply_type: ReplyType::Move,
            target_comment_id: Some(candidate.root_comment_id.clone()),
            body: limit_lines(
                "Thanks. This looks like a design/architecture discussion rather than a PR-local blocker.\nI propose moving it to ADR/RFC and keeping this PR scoped to the accepted behavior.",
                max_lines,
            ),
        };
    }

    DraftReplyArtifact {
        status: gate.status,
        reason: gate.reason.clone(),
        reply_type: ReplyType::Decline,
        target_comment_id: gate
            .classified_comments
            .first()
            .map(|comment| comment.comment.comment_id.clone()),
        body: limit_lines(
            "Thanks. I do not think this blocks merge for this PR.\nReason: the comment does not show a current PR risk.\nIf needed, I can track it separately.",
            max_lines,
        ),
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
