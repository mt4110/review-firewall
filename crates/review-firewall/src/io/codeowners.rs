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
    let tokens = trimmed.split_whitespace().collect::<Vec<_>>();
    if tokens.len() < 2 {
        return None;
    }
    Some(CodeownerRule {
        pattern: normalize_path(tokens[0]),
        owners: tokens[1..]
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
    })
}
