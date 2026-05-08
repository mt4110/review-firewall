use std::collections::BTreeMap;

use crate::domain::{ClassifiedComment, DuplicateGroup};
use crate::normalize::normalize_body;

pub fn collapse_duplicates(comments: &mut [ClassifiedComment]) -> Vec<DuplicateGroup> {
    let mut seen = BTreeMap::<String, String>::new();
    let mut grouped = BTreeMap::<String, Vec<String>>::new();

    for comment in comments.iter_mut() {
        let Some(key) = duplicate_key(comment) else {
            continue;
        };

        if let Some(primary) = seen.get(&key) {
            comment.duplicate_of_comment_id = Some(primary.clone());
            grouped
                .entry(primary.clone())
                .or_default()
                .push(comment.comment.comment_id.clone());
        } else {
            seen.insert(key, comment.comment.comment_id.clone());
        }
    }

    grouped
        .into_iter()
        .map(
            |(primary_comment_id, duplicate_comment_ids)| DuplicateGroup {
                primary_comment_id,
                duplicate_comment_ids,
            },
        )
        .collect()
}

fn duplicate_key(comment: &ClassifiedComment) -> Option<String> {
    let normalized = normalize_body(&comment.comment.body)
        .chars()
        .filter(|character| {
            character.is_ascii_alphanumeric() || *character == ' ' || *character == '/'
        })
        .collect::<String>();
    if normalized.len() < 24 {
        return None;
    }
    Some(format!(
        "{}|{}|{:?}|{}",
        comment.comment.thread_id,
        comment
            .comment
            .path
            .clone()
            .unwrap_or_else(|| "no-path".into()),
        comment.concern,
        normalized
    ))
}
