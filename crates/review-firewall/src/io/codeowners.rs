use std::fs;
use std::path::Path;

use rf_core::domain::CodeownerRule;
use rf_core::normalize_path;

#[derive(Debug, Clone)]
pub struct CodeownersFile {
    pub found: bool,
    pub rules: Vec<CodeownerRule>,
    pub reason: Option<String>,
}

pub fn load(repo_root: &Path) -> CodeownersFile {
    for path in codeowners_locations(repo_root) {
        match fs::read_to_string(&path) {
            Ok(content) => {
                let rules = content.lines().filter_map(parse_rule).collect::<Vec<_>>();
                return CodeownersFile {
                    found: true,
                    rules,
                    reason: None,
                };
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return CodeownersFile {
                    found: false,
                    rules: Vec::new(),
                    reason: Some(error.to_string()),
                };
            }
        }
    }

    CodeownersFile {
        found: false,
        rules: Vec::new(),
        reason: None,
    }
}

fn codeowners_locations(repo_root: &Path) -> [std::path::PathBuf; 3] {
    [
        repo_root.join(".github").join("CODEOWNERS"),
        repo_root.join("CODEOWNERS"),
        repo_root.join("docs").join("CODEOWNERS"),
    ]
}

fn parse_rule(line: &str) -> Option<CodeownerRule> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }
    let (pattern, owners_text) = split_pattern_and_owners(trimmed)?;
    let owners = owners_text
        .split_whitespace()
        .take_while(|value| !value.starts_with('#'))
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if owners.is_empty() {
        return None;
    }
    if !owners.iter().all(|owner| is_valid_owner(owner)) {
        return None;
    }
    Some(CodeownerRule {
        pattern: normalize_path(&pattern),
        owners,
    })
}

fn split_pattern_and_owners(line: &str) -> Option<(String, &str)> {
    let mut pattern = String::new();
    let mut escaped = false;

    for (index, character) in line.char_indices() {
        if escaped {
            if character.is_whitespace() {
                pattern.push(character);
            } else {
                pattern.push('\\');
                pattern.push(character);
            }
            escaped = false;
            continue;
        }

        if character == '\\' {
            escaped = true;
            continue;
        }

        if character.is_whitespace() {
            let owners = line[index..].trim();
            if pattern.is_empty() || owners.is_empty() {
                return None;
            }
            return Some((pattern, owners));
        }

        pattern.push(character);
    }

    if escaped {
        pattern.push('\\');
    }

    None
}

fn is_valid_owner(owner: &str) -> bool {
    if let Some(account) = owner.strip_prefix('@') {
        return is_valid_account_owner(account);
    }
    is_valid_email_owner(owner)
}

fn is_valid_account_owner(account: &str) -> bool {
    let parts = account.split('/').collect::<Vec<_>>();
    !parts.is_empty()
        && parts.len() <= 2
        && parts.iter().all(|part| {
            !part.is_empty()
                && part
                    .as_bytes()
                    .iter()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        })
}

fn is_valid_email_owner(owner: &str) -> bool {
    let Some((local, domain)) = owner.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && owner.as_bytes().iter().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'%' | b'+' | b'-' | b'@')
        })
}
