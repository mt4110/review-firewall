use crate::domain::{AdvisoryWeight, CodeownerRule, OwnershipAdvisory, OwnershipScope};
use crate::normalize::normalize_path;

pub fn build_ownership_advisory(
    reviewer: &str,
    comment_path: Option<&str>,
    changed_files: &[String],
    rules: &[CodeownerRule],
    use_codeowners: bool,
) -> OwnershipAdvisory {
    if !use_codeowners || reviewer.is_empty() || rules.is_empty() {
        return OwnershipAdvisory {
            owner_match: false,
            ownership_scope: OwnershipScope::None,
            advisory_weight: AdvisoryWeight::Low,
        };
    }

    if let Some(path) = comment_path
        && let Some(rule) = find_matching_rule(rules, path)
        && rule
            .owners
            .iter()
            .any(|owner| owner_matches(owner, reviewer))
    {
        return OwnershipAdvisory {
            owner_match: true,
            ownership_scope: OwnershipScope::Exact,
            advisory_weight: AdvisoryWeight::High,
        };
    }

    let partial = changed_files.iter().any(|path| {
        find_matching_rule(rules, path)
            .map(|rule| {
                rule.owners
                    .iter()
                    .any(|owner| owner_matches(owner, reviewer))
            })
            .unwrap_or(false)
    });

    if partial {
        OwnershipAdvisory {
            owner_match: true,
            ownership_scope: OwnershipScope::Partial,
            advisory_weight: AdvisoryWeight::Medium,
        }
    } else {
        OwnershipAdvisory {
            owner_match: false,
            ownership_scope: OwnershipScope::None,
            advisory_weight: AdvisoryWeight::Low,
        }
    }
}

pub fn find_matching_rule<'a>(
    rules: &'a [CodeownerRule],
    candidate_path: &str,
) -> Option<&'a CodeownerRule> {
    let normalized_path = normalize_path(candidate_path);
    let mut matched = None;
    for rule in rules {
        if codeowner_pattern_matches(&rule.pattern, &normalized_path) {
            matched = Some(rule);
        }
    }
    matched
}

fn owner_matches(owner: &str, reviewer: &str) -> bool {
    let normalized_owner = owner.trim().to_ascii_lowercase();
    let normalized_reviewer = reviewer.trim().to_ascii_lowercase();
    normalized_owner == normalized_reviewer || normalized_owner == format!("@{normalized_reviewer}")
}

fn codeowner_pattern_matches(pattern: &str, candidate_path: &str) -> bool {
    let normalized_pattern = normalize_path(pattern);
    let anchored = normalized_pattern.starts_with('/');
    let stripped = normalized_pattern.trim_start_matches('/');
    let pattern = if stripped.ends_with('/') {
        format!("{stripped}**")
    } else {
        stripped.to_owned()
    };

    if anchored {
        glob_matches(&pattern, candidate_path)
    } else {
        if glob_matches(&pattern, candidate_path) {
            return true;
        }

        let segments = candidate_path.split('/').collect::<Vec<_>>();
        for index in 1..segments.len() {
            let suffix = segments[index..].join("/");
            if glob_matches(&pattern, &suffix) {
                return true;
            }
        }
        false
    }
}

fn glob_matches(pattern: &str, candidate: &str) -> bool {
    glob_match_bytes(pattern.as_bytes(), candidate.as_bytes())
}

fn glob_match_bytes(pattern: &[u8], candidate: &[u8]) -> bool {
    if pattern.is_empty() {
        return candidate.is_empty();
    }

    match pattern[0] {
        b'*' if pattern.get(1) == Some(&b'*') => {
            let rest = &pattern[2..];
            if glob_match_bytes(rest, candidate) {
                return true;
            }
            for index in 0..candidate.len() {
                if glob_match_bytes(rest, &candidate[index + 1..]) {
                    return true;
                }
            }
            false
        }
        b'*' => {
            let rest = &pattern[1..];
            if glob_match_bytes(rest, candidate) {
                return true;
            }
            for index in 0..candidate.len() {
                if candidate[index] == b'/' {
                    break;
                }
                if glob_match_bytes(rest, &candidate[index + 1..]) {
                    return true;
                }
            }
            false
        }
        b'?' => {
            !candidate.is_empty()
                && candidate[0] != b'/'
                && glob_match_bytes(&pattern[1..], &candidate[1..])
        }
        value => {
            !candidate.is_empty()
                && value == candidate[0]
                && glob_match_bytes(&pattern[1..], &candidate[1..])
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::CodeownerRule;

    use super::build_ownership_advisory;

    #[test]
    fn exact_match_is_high_weight() {
        let advisory = build_ownership_advisory(
            "alice",
            Some("src/api.rs"),
            &[String::from("src/api.rs")],
            &[CodeownerRule {
                pattern: "/src/*".into(),
                owners: vec!["@alice".into()],
            }],
            true,
        );
        assert!(advisory.owner_match);
        assert_eq!(format!("{:?}", advisory.ownership_scope), "Exact");
    }

    #[test]
    fn non_anchored_rule_matches_nested_path() {
        let advisory = build_ownership_advisory(
            "alice",
            Some("nested/src/api.rs"),
            &[String::from("nested/src/api.rs")],
            &[CodeownerRule {
                pattern: "src/*".into(),
                owners: vec!["@alice".into()],
            }],
            true,
        );
        assert!(advisory.owner_match);
    }
}
