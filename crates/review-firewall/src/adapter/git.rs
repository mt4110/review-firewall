use std::path::{Path, PathBuf};

use rf_core::normalize_path;

use super::run_process;

#[derive(Debug, Clone)]
pub struct PathProbe {
    pub path: PathBuf,
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StringProbe {
    pub value: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PathsProbe {
    pub paths: Vec<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RepositoryProbe {
    pub identity: Option<RepositoryIdentity>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RepositoryIdentity {
    pub host: String,
    pub full_name: String,
}

pub fn repo_root(cwd: &Path) -> PathProbe {
    let output = run_process(
        cwd,
        "git",
        &[String::from("rev-parse"), String::from("--show-toplevel")],
    );
    if output.success {
        PathProbe {
            path: PathBuf::from(output.stdout.trim()),
            reason: None,
        }
    } else {
        PathProbe {
            path: cwd.to_path_buf(),
            reason: Some(
                output
                    .reason
                    .unwrap_or_else(|| fallback_reason(output.stderr, "git rev-parse failed")),
            ),
        }
    }
}

pub fn current_branch(repo_root: &Path) -> StringProbe {
    let output = run_process(
        repo_root,
        "git",
        &[
            String::from("rev-parse"),
            String::from("--abbrev-ref"),
            String::from("HEAD"),
        ],
    );
    if output.success {
        StringProbe {
            value: output.stdout.trim().to_owned(),
            reason: None,
        }
    } else {
        StringProbe {
            value: String::from("unknown"),
            reason: Some(
                output
                    .reason
                    .unwrap_or_else(|| fallback_reason(output.stderr, "git branch lookup failed")),
            ),
        }
    }
}

pub fn repository_identity(repo_root: &Path) -> RepositoryProbe {
    let output = run_process(
        repo_root,
        "git",
        &[
            String::from("remote"),
            String::from("get-url"),
            String::from("origin"),
        ],
    );
    if !output.success {
        return RepositoryProbe {
            identity: None,
            reason: Some(
                output
                    .reason
                    .unwrap_or_else(|| fallback_reason(output.stderr, "git remote lookup failed")),
            ),
        };
    }

    let remote = output.stdout.trim();
    let parsed = parse_github_remote(remote);
    RepositoryProbe {
        identity: parsed,
        reason: None,
    }
}

pub fn changed_files(repo_root: &Path, base_branch: Option<&str>) -> PathsProbe {
    let mut attempts = Vec::<Vec<String>>::new();
    if let Some(base_branch) = base_branch {
        attempts.push(vec![
            String::from("diff"),
            String::from("--name-only"),
            format!("origin/{base_branch}...HEAD"),
        ]);
        attempts.push(vec![
            String::from("diff"),
            String::from("--name-only"),
            format!("{base_branch}...HEAD"),
        ]);
    }
    attempts.push(vec![
        String::from("diff"),
        String::from("--name-only"),
        String::from("HEAD~1"),
        String::from("HEAD"),
    ]);
    attempts.push(vec![String::from("status"), String::from("--short")]);

    for attempt in attempts {
        let output = run_process(repo_root, "git", &attempt);
        if !output.success {
            continue;
        }
        let paths = if attempt.first().map(String::as_str) == Some("status") {
            parse_status_paths(&output.stdout)
        } else {
            parse_changed_paths(&output.stdout)
        };
        if !paths.is_empty() || attempt.first().map(String::as_str) == Some("status") {
            return PathsProbe {
                paths,
                reason: None,
            };
        }
    }

    PathsProbe {
        paths: Vec::new(),
        reason: Some(String::from("No local changed files detected")),
    }
}

fn parse_github_remote(remote: &str) -> Option<RepositoryIdentity> {
    let remote = remote.trim();
    let (host, suffix) = parse_remote_host_and_path(remote)?;
    let suffix = suffix.trim_start_matches([':', '/']);
    let suffix = suffix.strip_suffix(".git").unwrap_or(suffix);
    let mut parts = suffix
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.len() < 2 {
        return None;
    }
    let name = parts.pop()?;
    let owner = parts.pop()?;
    Some(RepositoryIdentity {
        host: host.to_owned(),
        full_name: format!("{owner}/{name}"),
    })
}

#[cfg(test)]
#[allow(dead_code)]
pub fn parse_github_remote_for_tests(remote: &str) -> Option<RepositoryIdentity> {
    parse_github_remote(remote)
}

fn parse_remote_host_and_path(remote: &str) -> Option<(&str, &str)> {
    if let Some((_, remainder)) = remote.split_once("://") {
        let (authority, path) = remainder.split_once('/')?;
        return Some((parse_remote_host(authority), path));
    }

    if let Some((authority, path)) = remote.split_once(':')
        && authority.contains('@')
        && !path.is_empty()
    {
        return Some((parse_remote_host(authority), path));
    }

    None
}

fn parse_remote_host(authority: &str) -> &str {
    let without_user = authority.rsplit('@').next().unwrap_or(authority);
    without_user
        .split_once(':')
        .map(|(host, _)| host)
        .unwrap_or(without_user)
}

fn parse_changed_paths(output: &str) -> Vec<String> {
    unique_paths(
        output
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(normalize_path)
            .collect(),
    )
}

fn parse_status_paths(output: &str) -> Vec<String> {
    let mut paths = Vec::new();
    for line in output.lines() {
        let line = line.trim_end();
        if line.len() < 4 {
            continue;
        }
        let candidate = line[3..].trim();
        if candidate.is_empty() {
            continue;
        }
        let path = candidate
            .split(" -> ")
            .last()
            .map(normalize_path)
            .unwrap_or_else(|| normalize_path(candidate));
        paths.push(path);
    }
    unique_paths(paths)
}

#[cfg(test)]
#[allow(dead_code)]
pub fn parse_status_paths_for_tests(output: &str) -> Vec<String> {
    parse_status_paths(output)
}

fn unique_paths(paths: Vec<String>) -> Vec<String> {
    let mut unique = Vec::new();
    for path in paths {
        if !unique.iter().any(|existing| existing == &path) {
            unique.push(path);
        }
    }
    unique
}

fn fallback_reason(stderr: String, fallback: &str) -> String {
    let trimmed = stderr.trim();
    if trimmed.is_empty() {
        fallback.to_owned()
    } else {
        trimmed.to_owned()
    }
}
