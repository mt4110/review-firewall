use std::collections::{BTreeMap, HashMap};

use crate::domain::{CommentRecord, ReviewThread};

pub fn normalize_path(input: &str) -> String {
    input.replace('\\', "/")
}

pub fn normalize_body(body: &str) -> String {
    body.to_lowercase()
        .replace('`', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn split_sentences(body: &str) -> Vec<String> {
    body.replace('\r', "\n")
        .split('\n')
        .flat_map(|line| line.split_terminator(['.', '!', '?']))
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

pub fn build_conversation_threads(
    review_comments: &[CommentRecord],
    issue_comments: &[CommentRecord],
) -> Vec<ReviewThread> {
    build_conversation_threads_for_author(review_comments, issue_comments, None)
}

pub fn build_conversation_threads_for_author(
    review_comments: &[CommentRecord],
    issue_comments: &[CommentRecord],
    pr_author: Option<&str>,
) -> Vec<ReviewThread> {
    let mut threads = build_review_threads_for_author(review_comments, pr_author);
    threads.extend(build_issue_comment_threads(issue_comments, pr_author));
    threads
}

pub fn build_review_threads(comments: &[CommentRecord]) -> Vec<ReviewThread> {
    build_review_threads_for_author(comments, None)
}

pub fn build_review_threads_for_author(
    comments: &[CommentRecord],
    pr_author: Option<&str>,
) -> Vec<ReviewThread> {
    let mut by_id = HashMap::new();
    for comment in comments {
        by_id.insert(comment.comment_id.clone(), comment.clone());
    }

    let mut grouped = BTreeMap::<String, Vec<CommentRecord>>::new();
    for comment in comments {
        let root_id = resolve_root_comment_id(comment, &by_id);
        let mut threaded = comment.clone();
        threaded.thread_id = root_id.clone();
        grouped.entry(root_id).or_default().push(threaded);
    }

    grouped
        .into_iter()
        .map(|(thread_id, mut thread_comments)| {
            thread_comments.sort_by(sort_comment_records);
            let participants = unique_participants(&thread_comments);
            let path = thread_comments
                .iter()
                .find_map(|comment| comment.path.clone());
            ReviewThread {
                thread_id: thread_id.clone(),
                root_comment_id: thread_id,
                path,
                participants,
                roundtrips: count_roundtrips(&thread_comments, pr_author),
                comments: thread_comments,
            }
        })
        .collect()
}

fn build_issue_comment_threads(
    comments: &[CommentRecord],
    pr_author: Option<&str>,
) -> Vec<ReviewThread> {
    let mut grouped = BTreeMap::<String, Vec<CommentRecord>>::new();
    for comment in comments {
        let thread_id =
            fallback_thread_id(comment).unwrap_or_else(|| format!("issue:{}", comment.comment_id));
        let mut threaded = comment.clone();
        threaded.thread_id = thread_id.clone();
        grouped.entry(thread_id).or_default().push(threaded);
    }

    grouped
        .into_iter()
        .map(|(thread_id, mut thread_comments)| {
            thread_comments.sort_by(sort_comment_records);
            let root_comment_id = thread_comments
                .first()
                .map(|comment| comment.comment_id.clone())
                .unwrap_or_else(|| thread_id.trim_start_matches("issue:").to_owned());
            ReviewThread {
                thread_id,
                root_comment_id,
                path: None,
                participants: unique_participants(&thread_comments),
                roundtrips: count_roundtrips(&thread_comments, pr_author),
                comments: thread_comments,
            }
        })
        .collect()
}

fn resolve_root_comment_id(
    comment: &CommentRecord,
    by_id: &HashMap<String, CommentRecord>,
) -> String {
    let mut current = comment;
    let mut visited = Vec::new();
    while let Some(reply_to) = current.reply_to_comment_id.as_ref() {
        if visited.iter().any(|seen: &String| seen == reply_to) {
            break;
        }
        let Some(parent) = by_id.get(reply_to) else {
            return fallback_thread_id(comment).unwrap_or_else(|| current.comment_id.clone());
        };
        visited.push(reply_to.clone());
        current = parent;
    }
    current.comment_id.clone()
}

fn fallback_thread_id(comment: &CommentRecord) -> Option<String> {
    let thread_id = comment.thread_id.trim();
    if thread_id.is_empty() {
        None
    } else {
        Some(thread_id.to_owned())
    }
}

fn sort_comment_records(left: &CommentRecord, right: &CommentRecord) -> std::cmp::Ordering {
    match left.created_at.cmp(&right.created_at) {
        std::cmp::Ordering::Equal => left.comment_id.cmp(&right.comment_id),
        order => order,
    }
}

fn unique_participants(comments: &[CommentRecord]) -> Vec<String> {
    let mut participants = Vec::new();
    for comment in comments {
        if !comment.author.is_empty()
            && !participants.iter().any(|author| author == &comment.author)
        {
            participants.push(comment.author.clone());
        }
    }
    participants
}

fn count_roundtrips(comments: &[CommentRecord], pr_author: Option<&str>) -> usize {
    let Some(pr_author) = pr_author.map(str::trim).filter(|author| !author.is_empty()) else {
        return 0;
    };

    let mut previous = None::<CommentSide>;
    let mut roundtrips = 0;
    for comment in comments {
        let Some(side) = comment_side(&comment.author, pr_author) else {
            continue;
        };
        if let Some(previous_side) = previous
            && previous_side != side
        {
            roundtrips += 1;
        }
        previous = Some(side);
    }
    roundtrips
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommentSide {
    PullRequestAuthor,
    Reviewer,
}

fn comment_side(author: &str, pr_author: &str) -> Option<CommentSide> {
    let author = author.trim();
    if author.is_empty() {
        return None;
    }
    if author.eq_ignore_ascii_case(pr_author) {
        Some(CommentSide::PullRequestAuthor)
    } else {
        Some(CommentSide::Reviewer)
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::{CommentRecord, CommentSource};

    use super::{
        build_conversation_threads, build_conversation_threads_for_author, build_review_threads,
        build_review_threads_for_author, normalize_path,
    };

    #[test]
    fn normalizes_windows_paths() {
        assert_eq!(normalize_path(r"src\main.rs"), "src/main.rs");
    }

    #[test]
    fn rebuilds_threads_from_reply_chain() {
        let comments = vec![
            CommentRecord {
                comment_id: "1".into(),
                thread_id: String::new(),
                author: "reviewer".into(),
                body: "This can break".into(),
                path: Some("src/lib.rs".into()),
                source: CommentSource::ReviewComment,
                reply_to_comment_id: None,
                created_at: Some("2026-03-28T00:00:00Z".into()),
                line: Some(1),
                original_line: Some(1),
            },
            CommentRecord {
                comment_id: "2".into(),
                thread_id: String::new(),
                author: "author".into(),
                body: "I disagree".into(),
                path: Some("src/lib.rs".into()),
                source: CommentSource::ReviewComment,
                reply_to_comment_id: Some("1".into()),
                created_at: Some("2026-03-28T00:00:01Z".into()),
                line: Some(1),
                original_line: Some(1),
            },
        ];
        let threads = build_review_threads(&comments);
        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0].root_comment_id, "1");
        assert_eq!(threads[0].roundtrips, 0);
    }

    #[test]
    fn preserves_normalized_thread_id_when_parent_is_missing() {
        let comments = vec![
            CommentRecord {
                comment_id: "1".into(),
                thread_id: "1".into(),
                author: "reviewer".into(),
                body: "This can break".into(),
                path: Some("src/lib.rs".into()),
                source: CommentSource::ReviewComment,
                reply_to_comment_id: None,
                created_at: Some("2026-03-28T00:00:00Z".into()),
                line: Some(1),
                original_line: Some(1),
            },
            CommentRecord {
                comment_id: "3".into(),
                thread_id: "1".into(),
                author: "author".into(),
                body: "Reply whose immediate parent is outside this page.".into(),
                path: Some("src/lib.rs".into()),
                source: CommentSource::ReviewComment,
                reply_to_comment_id: Some("2".into()),
                created_at: Some("2026-03-28T00:00:02Z".into()),
                line: Some(1),
                original_line: Some(1),
            },
        ];

        let threads = build_review_threads(&comments);

        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0].root_comment_id, "1");
        assert_eq!(
            threads[0]
                .comments
                .iter()
                .map(|comment| comment.comment_id.as_str())
                .collect::<Vec<_>>(),
            vec!["1", "3"]
        );
    }

    #[test]
    fn counts_author_reviewer_handoffs_when_author_is_known() {
        let comments = vec![
            review_comment("1", "reviewer-a", None, "2026-03-28T00:00:00Z"),
            review_comment("2", "reviewer-b", Some("1"), "2026-03-28T00:00:01Z"),
            review_comment("3", "author", Some("1"), "2026-03-28T00:00:02Z"),
            review_comment("4", "reviewer-c", Some("1"), "2026-03-28T00:00:03Z"),
            review_comment("5", "reviewer-d", Some("1"), "2026-03-28T00:00:04Z"),
        ];

        let threads = build_review_threads_for_author(&comments, Some("author"));

        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0].roundtrips, 2);
    }

    #[test]
    fn does_not_infer_roundtrips_without_pr_author() {
        let comments = vec![
            review_comment("1", "reviewer-a", None, "2026-03-28T00:00:00Z"),
            review_comment("2", "reviewer-b", Some("1"), "2026-03-28T00:00:01Z"),
            review_comment("3", "reviewer-c", Some("1"), "2026-03-28T00:00:02Z"),
        ];

        let threads = build_review_threads(&comments);

        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0].roundtrips, 0);
    }

    #[test]
    fn keeps_issue_comments_as_independent_pseudo_threads() {
        let issue_comments = vec![
            CommentRecord {
                comment_id: "10".into(),
                thread_id: String::new(),
                author: "reviewer".into(),
                body: "This contract discussion should move to an ADR.".into(),
                path: None,
                source: CommentSource::IssueComment,
                reply_to_comment_id: None,
                created_at: Some("2026-03-28T00:00:00Z".into()),
                line: None,
                original_line: None,
            },
            CommentRecord {
                comment_id: "11".into(),
                thread_id: String::new(),
                author: "author".into(),
                body: "I still think the schema belongs in this PR.".into(),
                path: None,
                source: CommentSource::IssueComment,
                reply_to_comment_id: None,
                created_at: Some("2026-03-28T00:00:01Z".into()),
                line: None,
                original_line: None,
            },
        ];

        let threads = build_conversation_threads(&[], &issue_comments);

        assert_eq!(threads.len(), 2);
        assert_eq!(threads[0].thread_id, "issue:10");
        assert_eq!(threads[0].roundtrips, 0);
        assert_eq!(threads[0].participants, vec!["reviewer"]);
        assert_eq!(threads[1].thread_id, "issue:11");
        assert_eq!(threads[1].roundtrips, 0);
        assert_eq!(threads[1].participants, vec!["author"]);
    }

    #[test]
    fn issue_comment_roundtrips_use_author_reviewer_handoffs_when_thread_id_is_shared() {
        let mut issue_comments = vec![
            issue_comment("10", "reviewer-a", "2026-03-28T00:00:00Z"),
            issue_comment("11", "reviewer-b", "2026-03-28T00:00:01Z"),
            issue_comment("12", "author", "2026-03-28T00:00:02Z"),
            issue_comment("13", "reviewer-c", "2026-03-28T00:00:03Z"),
        ];
        for comment in &mut issue_comments {
            comment.thread_id = String::from("issue:contract");
        }

        let threads = build_conversation_threads_for_author(&[], &issue_comments, Some("author"));

        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0].roundtrips, 2);
    }

    fn review_comment(
        comment_id: &str,
        author: &str,
        reply_to_comment_id: Option<&str>,
        created_at: &str,
    ) -> CommentRecord {
        CommentRecord {
            comment_id: comment_id.into(),
            thread_id: comment_id.into(),
            author: author.into(),
            body: "Body".into(),
            path: Some("src/lib.rs".into()),
            source: CommentSource::ReviewComment,
            reply_to_comment_id: reply_to_comment_id.map(Into::into),
            created_at: Some(created_at.into()),
            line: Some(1),
            original_line: Some(1),
        }
    }

    fn issue_comment(comment_id: &str, author: &str, created_at: &str) -> CommentRecord {
        CommentRecord {
            comment_id: comment_id.into(),
            thread_id: String::new(),
            author: author.into(),
            body: "Issue body".into(),
            path: None,
            source: CommentSource::IssueComment,
            reply_to_comment_id: None,
            created_at: Some(created_at.into()),
            line: None,
            original_line: None,
        }
    }
}
