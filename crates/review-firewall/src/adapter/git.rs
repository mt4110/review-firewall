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
        &[String::from("remote"), String::from("-v")],
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

    let parsed = parse_repository_identity_from_remotes(&output.stdout);
    let reason = if parsed.is_some() {
        None
    } else if output.stdout.trim().is_empty() {
        Some(String::from("No git remotes configured"))
    } else {
        Some(String::from(
            "Could not parse GitHub repository identity from git remotes",
        ))
    };
    RepositoryProbe {
        identity: parsed,
        reason,
    }
}

pub fn changed_files(repo_root: &Path, base_branch: Option<&str>) -> PathsProbe {
    let mut paths = Vec::new();
    let status_attempt = vec![
        String::from("status"),
        String::from("--porcelain=v1"),
        String::from("-z"),
        String::from("--untracked-files=all"),
    ];
    let status_output = run_process(repo_root, "git", &status_attempt);
    let status_reason = if status_output.success {
        merge_paths(&mut paths, parse_status_paths(&status_output.stdout));
        None
    } else {
        Some(
            status_output
                .reason
                .unwrap_or_else(|| fallback_reason(status_output.stderr, "git status failed")),
        )
    };

    let mut base_attempts = Vec::<Vec<String>>::new();
    if let Some(base_branch) = base_branch {
        base_attempts.push(vec![
            String::from("diff"),
            String::from("--name-only"),
            format!("origin/{base_branch}...HEAD"),
        ]);
        base_attempts.push(vec![
            String::from("diff"),
            String::from("--name-only"),
            format!("{base_branch}...HEAD"),
        ]);
    }
    for attempt in base_attempts {
        let output = run_process(repo_root, "git", &attempt);
        if output.success {
            merge_paths(&mut paths, parse_changed_paths(&output.stdout));
        }
    }

    if !paths.is_empty() {
        return PathsProbe {
            paths,
            reason: None,
        };
    }

    PathsProbe {
        paths: Vec::new(),
        reason: status_reason,
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

fn parse_repository_identity_from_remotes(output: &str) -> Option<RepositoryIdentity> {
    let remotes = output
        .lines()
        .filter_map(parse_remote_listing)
        .collect::<Vec<_>>();
    remotes
        .iter()
        .find(|remote| remote.name == "origin" && remote.direction == "(fetch)")
        .and_then(|remote| parse_github_remote(remote.url))
        .or_else(|| {
            remotes
                .iter()
                .find(|remote| remote.name == "origin")
                .and_then(|remote| parse_github_remote(remote.url))
        })
        .or_else(|| {
            remotes
                .iter()
                .find(|remote| remote.direction == "(fetch)")
                .and_then(|remote| parse_github_remote(remote.url))
        })
        .or_else(|| {
            remotes
                .iter()
                .find_map(|remote| parse_github_remote(remote.url))
        })
}

#[derive(Debug, Clone, Copy)]
struct RemoteListing<'a> {
    name: &'a str,
    url: &'a str,
    direction: &'a str,
}

fn parse_remote_listing(line: &str) -> Option<RemoteListing<'_>> {
    let mut parts = line.split_whitespace();
    Some(RemoteListing {
        name: parts.next()?,
        url: parts.next()?,
        direction: parts.next().unwrap_or(""),
    })
}

#[cfg(test)]
#[allow(dead_code)]
pub fn parse_github_remote_for_tests(remote: &str) -> Option<RepositoryIdentity> {
    parse_github_remote(remote)
}

#[cfg(test)]
#[allow(dead_code)]
pub fn parse_repository_identity_from_remotes_for_tests(
    output: &str,
) -> Option<RepositoryIdentity> {
    parse_repository_identity_from_remotes(output)
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
    let records = output
        .split('\0')
        .filter(|record| !record.is_empty())
        .collect::<Vec<_>>();
    let mut index = 0;
    while let Some(record) = records.get(index) {
        let status = record.get(..2).unwrap_or_default();
        let candidate = record.get(3..).unwrap_or_default();
        if candidate.is_empty() {
            index += 1;
            continue;
        }
        paths.push(normalize_path(candidate));
        index += if status.contains('R') || status.contains('C') {
            2
        } else {
            1
        };
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

fn merge_paths(paths: &mut Vec<String>, supplemental: Vec<String>) {
    for path in supplemental {
        if !paths.iter().any(|existing| existing == &path) {
            paths.push(path);
        }
    }
}

fn fallback_reason(stderr: String, fallback: &str) -> String {
    let trimmed = stderr.trim();
    if trimmed.is_empty() {
        fallback.to_owned()
    } else {
        trimmed.to_owned()
    }
}
