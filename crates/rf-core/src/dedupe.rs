use std::collections::BTreeMap;

use crate::domain::{ClassifiedComment, DuplicateGroup};
use crate::normalize::normalize_body;

pub fn collapse_duplicates(comments: &mut [ClassifiedComment]) -> Vec<DuplicateGroup> {
    collapse_duplicates_with_primary_filter(comments, |_| true)
}

pub fn collapse_duplicates_with_primary_filter<F>(
    comments: &mut [ClassifiedComment],
    can_be_primary: F,
) -> Vec<DuplicateGroup>
where
    F: Fn(&ClassifiedComment) -> bool,
{
    let mut grouped = BTreeMap::<String, Vec<usize>>::new();

    for comment in comments.iter_mut() {
        comment.duplicate_of_comment_id = None;
    }

    for (index, comment) in comments.iter().enumerate() {
        let Some(key) = duplicate_key(comment) else {
            continue;
        };

        grouped.entry(key).or_default().push(index);
    }

    let mut collapsed = Vec::new();
    for indexes in grouped.values() {
        if indexes.len() < 2 {
            continue;
        }
        let primary_index = indexes
            .iter()
            .copied()
            .find(|index| can_be_primary(&comments[*index]))
            .unwrap_or(indexes[0]);
        let primary_comment_id = comments[primary_index].comment.comment_id.clone();
        let mut duplicate_comment_ids = Vec::new();

        for index in indexes
            .iter()
            .copied()
            .filter(|index| *index != primary_index)
        {
            comments[index].duplicate_of_comment_id = Some(primary_comment_id.clone());
            duplicate_comment_ids.push(comments[index].comment.comment_id.clone());
        }

        collapsed.push(DuplicateGroup {
            primary_comment_id,
            duplicate_comment_ids,
        });
    }

    collapsed
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
