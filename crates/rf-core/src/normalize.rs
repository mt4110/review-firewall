use std::collections::{BTreeMap, HashMap};

use crate::domain::{CommentRecord, ReviewThread};

pub fn normalize_path(input: &str) -> String {
    input.replace('\\', "/")
}

pub fn normalize_body(body: &str) -> String {
    normalize_analysis_text(&clean_analysis_text(body))
}

pub fn split_sentences(body: &str) -> Vec<String> {
    clean_analysis_text(body)
        .replace('\r', "\n")
        .split('\n')
        .flat_map(|line| line.split_terminator(['.', '!', '?', '。', '！', '？']))
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn clean_analysis_text(body: &str) -> String {
    let without_code_fences = strip_fenced_code_blocks(body);
    let without_images = strip_markdown_images(&without_code_fences);
    let normalized_links = normalize_markdown_links(&without_images);
    let without_html = strip_html_tags(&normalized_links);
    let without_urls = strip_url_tokens(&without_html);

    without_urls
        .replace(['*', '_', '~', '#', '>'], " ")
        .replace(['[', ']', '(', ')'], " ")
}

fn normalize_analysis_text(body: &str) -> String {
    body.to_lowercase()
        .replace('`', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn strip_fenced_code_blocks(input: &str) -> String {
    let mut output = String::new();
    let mut remaining = input;
    let fence = "```";

    while let Some(start) = remaining.find(fence) {
        output.push_str(&remaining[..start]);
        remaining = &remaining[start + fence.len()..];
        if let Some(end) = remaining.find(fence) {
            remaining = &remaining[end + fence.len()..];
        } else {
            break;
        }
        output.push(' ');
    }

    output.push_str(remaining);
    output
}

fn strip_markdown_images(input: &str) -> String {
    let chars = input.chars().collect::<Vec<_>>();
    let mut output = String::new();
    let mut index = 0usize;

    while index < chars.len() {
        if chars[index] == '!'
            && index + 1 < chars.len()
            && chars[index + 1] == '['
            && let Some(next_index) = skip_markdown_destination(&chars, index + 1)
        {
            output.push(' ');
            index = next_index;
            continue;
        }

        output.push(chars[index]);
        index += 1;
    }

    output
}

fn normalize_markdown_links(input: &str) -> String {
    let chars = input.chars().collect::<Vec<_>>();
    let mut output = String::new();
    let mut index = 0usize;

    while index < chars.len() {
        if chars[index] == '['
            && let Some((label, next_index)) = extract_markdown_link_label(&chars, index)
            && !label.trim().is_empty()
        {
            output.push_str(label.trim());
            output.push(' ');
            index = next_index;
            continue;
        }

        output.push(chars[index]);
        index += 1;
    }

    output
}

fn extract_markdown_link_label(chars: &[char], start: usize) -> Option<(String, usize)> {
    let close_bracket = find_char(chars, start + 1, ']')?;
    if close_bracket + 1 >= chars.len() || chars[close_bracket + 1] != '(' {
        return None;
    }

    let close_paren = find_matching_paren(chars, close_bracket + 1)?;
    let label = chars[start + 1..close_bracket].iter().collect::<String>();

    Some((label, close_paren + 1))
}

fn skip_markdown_destination(chars: &[char], start_bracket: usize) -> Option<usize> {
    let close_bracket = find_char(chars, start_bracket + 1, ']')?;
    if close_bracket + 1 < chars.len() && chars[close_bracket + 1] == '(' {
        let close_paren = find_matching_paren(chars, close_bracket + 1)?;
        Some(close_paren + 1)
    } else {
        Some(close_bracket + 1)
    }
}

fn strip_html_tags(input: &str) -> String {
    let characters = input.chars().collect::<Vec<_>>();
    let mut output = String::new();
    let mut index = 0usize;

    while index < characters.len() {
        if characters[index] == '<'
            && let Some(close_index) = find_char(&characters, index + 1, '>')
            && is_probable_html_tag(&characters[index + 1..close_index])
        {
            output.push(' ');
            index = close_index + 1;
            continue;
        }

        output.push(characters[index]);
        index += 1;
    }

    output
}

fn strip_url_tokens(input: &str) -> String {
    input
        .lines()
        .map(|line| {
            line.split_whitespace()
                .filter(|token| !looks_like_url(token))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn looks_like_url(token: &str) -> bool {
    let trimmed = token.trim_matches(|character: char| ",.;:()[]{}".contains(character));
    trimmed.contains("://")
        || trimmed.starts_with("www.")
        || trimmed.contains("shields.io")
        || trimmed.contains("style=flat")
}

fn is_probable_html_tag(content: &[char]) -> bool {
    let raw = content.iter().collect::<String>();
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return false;
    }

    if trimmed.starts_with("!--") {
        return true;
    }

    let without_slash = trimmed.strip_prefix('/').unwrap_or(trimmed);
    let tag_name = without_slash
        .chars()
        .take_while(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || *character == '-'
        })
        .collect::<String>();

    if tag_name.is_empty() {
        return false;
    }

    without_slash[tag_name.len()..].chars().all(|character| {
        character.is_whitespace()
            || character.is_ascii_alphanumeric()
            || matches!(
                character,
                '/' | '=' | '"' | '\'' | ':' | '-' | '_' | '.' | '?' | '&' | '#' | '%' | ';'
            )
    })
}

fn find_char(chars: &[char], start: usize, needle: char) -> Option<usize> {
    chars[start..]
        .iter()
        .position(|character| *character == needle)
        .map(|offset| start + offset)
}

fn find_matching_paren(chars: &[char], open_paren: usize) -> Option<usize> {
    let mut depth = 0usize;

    for (index, character) in chars.iter().enumerate().skip(open_paren) {
        match character {
            '(' => depth += 1,
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }

    None
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
        let thread_id = issue_comment_thread_id(comment);
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

fn issue_comment_thread_id(comment: &CommentRecord) -> String {
    let Some(thread_id) = fallback_thread_id(comment) else {
        return format!("issue:{}", comment.comment_id);
    };
    if thread_id == comment.comment_id || thread_id == format!("issue:{}", comment.comment_id) {
        format!("issue:{}", comment.comment_id)
    } else {
        thread_id
    }
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
    fn keeps_issue_comments_without_explicit_thread_id_as_pseudo_threads() {
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
