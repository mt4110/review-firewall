use crate::domain::{EscalationCandidate, EscalationLabel, ReviewThread, Status};

pub fn evaluate_escalation_candidates(
    review_threads: &[ReviewThread],
    max_roundtrips: usize,
) -> Vec<EscalationCandidate> {
    review_threads
        .iter()
        .filter(|thread| thread.roundtrips > max_roundtrips)
        .map(|thread| {
            let (label, why) = label_thread(thread);
            let (position_a, position_b) = positions(thread);
            EscalationCandidate {
                thread_id: thread.thread_id.clone(),
                root_comment_id: thread.root_comment_id.clone(),
                label,
                title: title(thread),
                path: thread.path.clone(),
                roundtrips: thread.roundtrips,
                participants: thread.participants.clone(),
                position_a,
                position_b,
                why,
            }
        })
        .collect()
}

pub fn build_escalation_markdown(
    status: Status,
    reason: Option<&str>,
    pr_number: Option<u64>,
    candidates: &[EscalationCandidate],
) -> String {
    let mut lines = vec![format!("STATUS: {}", status.terminal_label())];
    if let Some(reason) = reason
        && !reason.is_empty()
    {
        lines.push(format!("REASON: {reason}"));
    }
    lines.push(String::new());
    lines.push(String::from("# Escalation"));
    lines.push(String::new());

    let actionable = candidates
        .iter()
        .filter(|candidate| candidate.label != EscalationLabel::StayInPr)
        .collect::<Vec<_>>();

    if actionable.is_empty() {
        lines.push(String::from("No ADR/RFC candidates were found."));
        return lines.join("\n");
    }

    for candidate in actionable {
        lines.push(candidate_header(candidate.label).to_owned());
        lines.push(String::new());
        lines.push(String::from("## Title"));
        lines.push(candidate.title.clone());
        lines.push(String::new());
        lines.push(String::from("## Why this was escalated"));
        lines.push(candidate.why.clone());
        lines.push(String::new());
        lines.push(String::from("## Position A"));
        lines.push(candidate.position_a.clone());
        lines.push(String::new());
        lines.push(String::from("## Position B"));
        lines.push(candidate.position_b.clone());
        lines.push(String::new());
        lines.push(String::from("## Decision needed"));
        lines.push(String::from("- Is this a PR blocker?"));
        lines.push(String::from("- Which contract becomes source of truth?"));
        lines.push(String::from(
            "- Can current PR merge before this is decided?",
        ));
        lines.push(String::new());
        lines.push(String::from("## Related PR"));
        lines.push(
            pr_number
                .map(|value| format!("#{value}"))
                .unwrap_or_else(|| "unknown".into()),
        );
        lines.push(String::new());
    }

    lines.join("\n")
}

fn label_thread(thread: &ReviewThread) -> (EscalationLabel, String) {
    let text = thread
        .comments
        .iter()
        .map(|comment| comment.body.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join("\n");

    if contains_any(
        &text,
        &["null", "retry", "test", "bug", "migration", "rollback"],
    ) {
        return (
            EscalationLabel::StayInPr,
            String::from(
                "The thread is long, but it still reads like a PR-local bug or test discussion.",
            ),
        );
    }
    if contains_any(
        &text,
        &[
            "public api",
            "external api",
            "protocol",
            "cross-team",
            "consumer",
            "backward compatible",
        ],
    ) {
        return (
            EscalationLabel::MoveToRfc,
            String::from("The thread looks like a cross-boundary contract discussion."),
        );
    }
    if contains_any(
        &text,
        &[
            "boundary",
            "responsibility",
            "contract",
            "schema",
            "data model",
            "persistence",
            "architecture",
            "layer",
        ],
    ) {
        return (
            EscalationLabel::MoveToAdr,
            String::from("The thread has moved into design or architecture territory."),
        );
    }
    (
        EscalationLabel::NeedsHumanJudgment,
        String::from(
            "The thread exceeded the roundtrip threshold without a clear PR-local resolution.",
        ),
    )
}

fn candidate_header(label: EscalationLabel) -> &'static str {
    match label {
        EscalationLabel::MoveToAdr => "# ADR Candidate",
        EscalationLabel::MoveToRfc => "# RFC Candidate",
        EscalationLabel::NeedsHumanJudgment => "# Human Judgment Candidate",
        EscalationLabel::StayInPr => "# PR-local Candidate",
    }
}

fn title(thread: &ReviewThread) -> String {
    let text = thread
        .comments
        .iter()
        .map(|comment| comment.body.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join("\n");
    let basename = thread
        .path
        .as_deref()
        .and_then(|path| path.rsplit('/').next())
        .unwrap_or("review-thread");

    if contains_any(
        &text,
        &["contract", "schema", "response shape", "request shape"],
    ) {
        format!("API contract discussion in {basename}")
    } else if contains_any(&text, &["data model", "persistence", "database"]) {
        format!("Data model discussion in {basename}")
    } else if contains_any(&text, &["boundary", "responsibility", "architecture"]) {
        format!("Boundary discussion in {basename}")
    } else {
        format!("Review thread for {basename}")
    }
}

fn positions(thread: &ReviewThread) -> (String, String) {
    let mut unique = Vec::<String>::new();
    for comment in &thread.comments {
        let sentence = comment
            .body
            .split_terminator(['.', '!', '?'])
            .map(str::trim)
            .find(|line| !line.is_empty())
            .unwrap_or(comment.body.as_str());
        if !unique.iter().any(|item| item == sentence) {
            unique.push(sentence.to_owned());
        }
    }

    (
        unique
            .first()
            .cloned()
            .unwrap_or_else(|| String::from("Keep the current implementation in this PR.")),
        unique
            .get(1)
            .cloned()
            .unwrap_or_else(|| String::from("Change the design before merging this PR.")),
    )
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

#[cfg(test)]
mod tests {
    use crate::domain::{CommentRecord, CommentSource, EscalationLabel, ReviewThread, Status};

    use super::{build_escalation_markdown, evaluate_escalation_candidates};

    #[test]
    fn labels_contract_thread_for_adr() {
        let thread = ReviewThread {
            thread_id: "1".into(),
            root_comment_id: "1".into(),
            path: Some("src/api.rs".into()),
            participants: vec!["reviewer".into(), "author".into()],
            roundtrips: 3,
            comments: vec![CommentRecord {
                comment_id: "1".into(),
                thread_id: "1".into(),
                author: "reviewer".into(),
                body: "This API contract should change before merge.".into(),
                path: Some("src/api.rs".into()),
                source: CommentSource::ReviewComment,
                reply_to_comment_id: None,
                created_at: None,
                line: None,
                original_line: None,
            }],
        };
        let candidates = evaluate_escalation_candidates(&[thread], 2);
        assert_eq!(candidates[0].label, EscalationLabel::MoveToAdr);
    }

    #[test]
    fn labels_issue_comment_debate_for_adr() {
        let thread = ReviewThread {
            thread_id: "issue:21".into(),
            root_comment_id: "21".into(),
            path: None,
            participants: vec!["reviewer".into(), "author".into()],
            roundtrips: 3,
            comments: vec![
                CommentRecord {
                    comment_id: "21".into(),
                    thread_id: "issue:21".into(),
                    author: "reviewer".into(),
                    body: "This contract boundary should move to an ADR.".into(),
                    path: None,
                    source: CommentSource::IssueComment,
                    reply_to_comment_id: None,
                    created_at: None,
                    line: None,
                    original_line: None,
                },
                CommentRecord {
                    comment_id: "22".into(),
                    thread_id: "issue:21".into(),
                    author: "author".into(),
                    body: "I still think the architecture belongs in this PR.".into(),
                    path: None,
                    source: CommentSource::IssueComment,
                    reply_to_comment_id: None,
                    created_at: None,
                    line: None,
                    original_line: None,
                },
            ],
        };

        let candidates = evaluate_escalation_candidates(&[thread], 2);

        assert_eq!(candidates[0].label, EscalationLabel::MoveToAdr);
    }

    #[test]
    fn renders_rfc_candidate_header_for_cross_boundary_thread() {
        let thread = ReviewThread {
            thread_id: "rfc".into(),
            root_comment_id: "rfc".into(),
            path: Some("src/api.rs".into()),
            participants: vec!["reviewer".into(), "author".into()],
            roundtrips: 3,
            comments: vec![CommentRecord {
                comment_id: "rfc".into(),
                thread_id: "rfc".into(),
                author: "reviewer".into(),
                body: "This public API affects cross-team consumers.".into(),
                path: Some("src/api.rs".into()),
                source: CommentSource::ReviewComment,
                reply_to_comment_id: None,
                created_at: None,
                line: None,
                original_line: None,
            }],
        };
        let candidates = evaluate_escalation_candidates(&[thread], 2);

        let markdown = build_escalation_markdown(Status::Ok, None, Some(42), &candidates);

        assert!(markdown.contains("# RFC Candidate"));
        assert!(!markdown.contains("# ADR Candidate"));
    }

    #[test]
    fn renders_human_judgment_header_for_unclear_long_thread() {
        let thread = ReviewThread {
            thread_id: "human".into(),
            root_comment_id: "human".into(),
            path: Some("src/view.rs".into()),
            participants: vec!["reviewer".into(), "author".into()],
            roundtrips: 4,
            comments: vec![CommentRecord {
                comment_id: "human".into(),
                thread_id: "human".into(),
                author: "reviewer".into(),
                body: "I still do not feel good about this direction.".into(),
                path: Some("src/view.rs".into()),
                source: CommentSource::ReviewComment,
                reply_to_comment_id: None,
                created_at: None,
                line: None,
                original_line: None,
            }],
        };
        let candidates = evaluate_escalation_candidates(&[thread], 2);

        let markdown = build_escalation_markdown(Status::Ok, None, Some(42), &candidates);

        assert!(markdown.contains("# Human Judgment Candidate"));
        assert!(!markdown.contains("# ADR Candidate"));
    }
}
