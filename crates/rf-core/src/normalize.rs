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
    let mut threads = build_review_threads(review_comments);
    if let Some(issue_thread) = build_issue_comment_thread(issue_comments) {
        threads.push(issue_thread);
    }
    threads
}

pub fn build_review_threads(comments: &[CommentRecord]) -> Vec<ReviewThread> {
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
                roundtrips: count_roundtrips(&thread_comments),
                comments: thread_comments,
            }
        })
        .collect()
}

fn build_issue_comment_thread(comments: &[CommentRecord]) -> Option<ReviewThread> {
    let mut thread_comments = comments.to_vec();
    if thread_comments.is_empty() {
        return None;
    }

    thread_comments.sort_by(sort_comment_records);
    let root_comment_id = thread_comments.first()?.comment_id.clone();
    let thread_id = format!("issue:{root_comment_id}");
    for comment in &mut thread_comments {
        comment.thread_id = thread_id.clone();
    }

    Some(ReviewThread {
        thread_id,
        root_comment_id,
        path: None,
        participants: unique_participants(&thread_comments),
        roundtrips: count_roundtrips(&thread_comments),
        comments: thread_comments,
    })
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
            break;
        };
        visited.push(reply_to.clone());
        current = parent;
    }
    current.comment_id.clone()
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

fn count_roundtrips(comments: &[CommentRecord]) -> usize {
    let mut previous = String::new();
    let mut roundtrips = 0;
    for comment in comments {
        if comment.author.is_empty() {
            continue;
        }
        if previous.is_empty() {
            previous = comment.author.clone();
            continue;
        }
        if previous != comment.author {
            roundtrips += 1;
            previous = comment.author.clone();
        }
    }
    roundtrips
}

#[cfg(test)]
mod tests {
    use crate::domain::{CommentRecord, CommentSource};

    use super::{build_conversation_threads, build_review_threads, normalize_path};

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
        assert_eq!(threads[0].roundtrips, 1);
    }

    #[test]
    fn folds_issue_comments_into_a_pseudo_thread() {
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

        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0].thread_id, "issue:10");
        assert_eq!(threads[0].roundtrips, 1);
        assert_eq!(threads[0].participants, vec!["reviewer", "author"]);
    }
}
