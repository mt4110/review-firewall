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

pub fn head_oid(repo_root: &Path) -> StringProbe {
    let output = run_process(
        repo_root,
        "git",
        &[String::from("rev-parse"), String::from("HEAD")],
    );
    if output.success {
        StringProbe {
            value: output.stdout.trim().to_owned(),
            reason: None,
        }
    } else {
        StringProbe {
            value: String::new(),
            reason: Some(
                output
                    .reason
                    .unwrap_or_else(|| fallback_reason(output.stderr, "git HEAD lookup failed")),
            ),
        }
    }
}

pub fn fallback_base_branch(repo_root: &Path) -> StringProbe {
    let origin_head = run_process(
        repo_root,
        "git",
        &[
            String::from("symbolic-ref"),
            String::from("--quiet"),
            String::from("--short"),
            String::from("refs/remotes/origin/HEAD"),
        ],
    );
    if origin_head.success {
        let branch = origin_head
            .stdout
            .trim()
            .strip_prefix("origin/")
            .unwrap_or(origin_head.stdout.trim())
            .to_owned();
        if !branch.is_empty() {
            return StringProbe {
                value: branch,
                reason: None,
            };
        }
    }

    let candidates = ["main", "master", "develop", "development", "trunk"];
    for candidate in candidates {
        if ref_exists(repo_root, &format!("origin/{candidate}")) {
            return StringProbe {
                value: candidate.to_owned(),
                reason: None,
            };
        }
    }

    for candidate in candidates {
        if ref_exists(repo_root, candidate) {
            return StringProbe {
                value: candidate.to_owned(),
                reason: None,
            };
        }
    }

    let current_branch = current_branch(repo_root).value;
    if let Some(branch) =
        infer_unique_branch(repo_root, "refs/remotes/origin", Some(&current_branch))
            .or_else(|| infer_unique_branch(repo_root, "refs/heads", Some(&current_branch)))
    {
        return StringProbe {
            value: branch,
            reason: None,
        };
    }

    StringProbe {
        value: String::new(),
        reason: Some(String::from(
            "Could not infer a base branch for local changed-file diff",
        )),
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

    let base_reason = base_branch
        .filter(|value| !value.trim().is_empty())
        .and_then(|base_branch| {
            let base_probe = changed_files_against_base(repo_root, Some(base_branch));
            merge_paths(&mut paths, base_probe.paths);
            base_probe.reason
        });

    PathsProbe {
        paths,
        reason: combine_reasons(status_reason, base_reason),
    }
}

pub fn changed_files_against_base(repo_root: &Path, base_branch: Option<&str>) -> PathsProbe {
    let Some(base_branch) = base_branch.filter(|value| !value.trim().is_empty()) else {
        return PathsProbe {
            paths: Vec::new(),
            reason: Some(String::from(
                "Could not diff against base branch: base branch is unknown",
            )),
        };
    };

    let mut paths = Vec::new();
    let mut failed_reasons = Vec::new();
    for reference in [format!("origin/{base_branch}"), base_branch.to_owned()] {
        let attempt = vec![
            String::from("diff"),
            String::from("--name-only"),
            format!("{reference}...HEAD"),
        ];
        let output = run_process(repo_root, "git", &attempt);
        if output.success {
            merge_paths(&mut paths, parse_changed_paths(&output.stdout));
            return PathsProbe {
                paths,
                reason: None,
            };
        } else {
            failed_reasons.push(
                output
                    .reason
                    .unwrap_or_else(|| fallback_reason(output.stderr, "git diff failed")),
            );
        }
    }

    PathsProbe {
        paths,
        reason: Some(format!(
            "Could not diff against base branch '{}': {}",
            base_branch,
            summarize_reasons(&failed_reasons)
        )),
    }
}

fn ref_exists(repo_root: &Path, reference: &str) -> bool {
    let output = run_process(
        repo_root,
        "git",
        &[
            String::from("rev-parse"),
            String::from("--verify"),
            String::from("--quiet"),
            reference.to_owned(),
        ],
    );
    output.success
}

fn infer_unique_branch(
    repo_root: &Path,
    namespace: &str,
    current_branch: Option<&str>,
) -> Option<String> {
    let output = run_process(
        repo_root,
        "git",
        &[
            String::from("for-each-ref"),
            String::from("--format=%(refname:short)"),
            namespace.to_owned(),
        ],
    );
    if !output.success {
        return None;
    }

    let current_branch = current_branch.filter(|value| !value.trim().is_empty());
    let mut branches = Vec::new();
    for reference in output.stdout.lines().map(str::trim) {
        let Some(branch) = normalize_branch_reference(reference) else {
            continue;
        };
        if current_branch == Some(branch.as_str()) {
            continue;
        }
        if !branches.iter().any(|existing| existing == &branch) {
            branches.push(branch);
        }
    }

    if branches.len() == 1 {
        branches.pop()
    } else {
        None
    }
}

fn normalize_branch_reference(reference: &str) -> Option<String> {
    let branch = reference
        .strip_prefix("origin/")
        .unwrap_or(reference)
        .trim();
    if branch.is_empty() || branch == "HEAD" {
        None
    } else {
        Some(branch.to_owned())
    }
}

fn summarize_reasons(reasons: &[String]) -> String {
    reasons
        .iter()
        .map(|reason| reason.trim())
        .find(|reason| !reason.is_empty())
        .unwrap_or("git diff failed")
        .to_owned()
}

fn combine_reasons(first: Option<String>, second: Option<String>) -> Option<String> {
    match (first, second) {
        (Some(first), Some(second)) => Some(format!("{first}; {second}")),
        (Some(first), None) => Some(first),
        (None, Some(second)) => Some(second),
        (None, None) => None,
    }
}

fn parse_github_remote(remote: &str) -> Option<RepositoryIdentity> {
    let remote = remote.trim();
    let (host, suffix) = parse_remote_host_and_path(remote)?;
    if is_known_non_github_host(host) {
        return None;
    }
    let suffix = suffix.trim_start_matches([':', '/']);
    let suffix = suffix.trim_end_matches('/');
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

fn is_known_non_github_host(host: &str) -> bool {
    matches!(
        host_without_port(host).to_ascii_lowercase().as_str(),
        "gitlab.com" | "bitbucket.org" | "ssh.dev.azure.com" | "dev.azure.com"
    )
}

fn host_without_port(host: &str) -> &str {
    if host.starts_with('[') {
        return host;
    }
    host.split_once(':').map(|(host, _)| host).unwrap_or(host)
}

fn parse_repository_identity_from_remotes(output: &str) -> Option<RepositoryIdentity> {
    let remotes = output
        .lines()
        .filter_map(parse_remote_listing)
        .collect::<Vec<_>>();
    find_repository_identity(&remotes)
}

fn find_repository_identity(remotes: &[RemoteListing<'_>]) -> Option<RepositoryIdentity> {
    find_remote_identity(remotes, |remote| {
        remote.name == "origin" && remote.direction == "(fetch)"
    })
    .or_else(|| find_remote_identity(remotes, |remote| remote.name == "origin"))
    .or_else(|| find_remote_identity(remotes, |remote| remote.direction == "(fetch)"))
    .or_else(|| find_remote_identity(remotes, |_| true))
}

fn find_remote_identity(
    remotes: &[RemoteListing<'_>],
    predicate: impl Fn(&RemoteListing<'_>) -> bool,
) -> Option<RepositoryIdentity> {
    remotes
        .iter()
        .filter(|remote| predicate(remote))
        .filter_map(|remote| parse_github_remote(remote.url))
        .next()
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
        && !authority.is_empty()
        && !path.is_empty()
        && path.contains('/')
        && !authority.contains(['/', '\\'])
    {
        return Some((parse_remote_host(authority), path));
    }

    None
}

fn parse_remote_host(authority: &str) -> &str {
    authority.rsplit('@').next().unwrap_or(authority)
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
