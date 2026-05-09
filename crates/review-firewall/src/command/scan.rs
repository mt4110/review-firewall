use std::collections::HashMap;
use std::path::Path;

use rf_core::domain::{
    CommentRecord, CommentSource, ProductBoundarySnapshot, PullRequestSummary, ScanArtifact, Status,
};
use rf_core::{build_conversation_threads_for_author, normalize_path};
use serde_json::Value;

use crate::adapter::{gh, git};
use crate::command::CommandOutcome;
use crate::io::{artifacts, codeowners, config, run_store};

const ISSUE_CONVERSATION_THREAD_ID: &str = "issue:conversation";

pub fn run(cwd: &Path, pr_override: Option<u64>) -> Result<CommandOutcome, String> {
    let repo_root = git::repo_root(cwd);
    let run = run_store::create_new(&repo_root.path).map_err(io_error)?;
    let policy = config::load(&repo_root.path);
    let codeowners_file = codeowners::load(&repo_root.path);
    let branch = git::current_branch(&repo_root.path);

    let mut status = Status::Ok;
    let mut reason = None::<String>;
    let mut warnings = Vec::new();
    let mut partial_sources = Vec::new();

    merge_probe_reason(
        &mut status,
        &mut reason,
        &mut warnings,
        repo_root.reason,
        Status::Partial,
    );
    merge_probe_reason(
        &mut status,
        &mut reason,
        &mut warnings,
        branch.reason,
        Status::Partial,
    );
    merge_probe_reason(
        &mut status,
        &mut reason,
        &mut warnings,
        policy.reason.clone(),
        policy.status,
    );
    merge_probe_reason(
        &mut status,
        &mut reason,
        &mut warnings,
        codeowners_file.reason.clone(),
        Status::Partial,
    );

    let pr_view = gh::pr_view(&repo_root.path, pr_override);
    let mut pr = PullRequestSummary::default();
    let mut changed_files;
    let mut comments = Vec::<CommentRecord>::new();
    let mut issue_comments = Vec::<CommentRecord>::new();
    let mut review_threads = Vec::new();

    match pr_view {
        Ok(pr_data) => {
            pr = build_pull_request_summary(&pr_data);
            changed_files = pr_changed_files(&pr_data);
            let repository = repository_identity_from_pr_url(&pr_data)
                .map(|identity| git::RepositoryProbe {
                    identity: Some(identity),
                    reason: None,
                })
                .unwrap_or_else(|| git::repository_identity(&repo_root.path));

            let local_changed = git::changed_files(&repo_root.path, pr.base_branch.as_deref());
            changed_files = merge_changed_files(changed_files, &local_changed.paths);
            merge_probe_reason(
                &mut status,
                &mut reason,
                &mut warnings,
                local_changed.reason,
                Status::Partial,
            );

            if let (Some(pr_number), Some(repository)) = (pr.number, repository.identity.clone()) {
                match gh::changed_files(
                    &repo_root.path,
                    &repository.full_name,
                    repository.host.as_str(),
                    pr_number,
                ) {
                    Ok(probe) => {
                        changed_files =
                            merge_changed_files(changed_files, &api_changed_files(&probe.values));
                        if let Some(error) = probe.reason {
                            partial_sources.push(String::from("changed_files"));
                            merge_probe_reason(
                                &mut status,
                                &mut reason,
                                &mut warnings,
                                Some(error),
                                Status::Partial,
                            );
                        }
                    }
                    Err(error) => {
                        partial_sources.push(String::from("changed_files"));
                        merge_probe_reason(
                            &mut status,
                            &mut reason,
                            &mut warnings,
                            Some(error),
                            Status::Partial,
                        );
                    }
                }

                match gh::review_comments(
                    &repo_root.path,
                    &repository.full_name,
                    repository.host.as_str(),
                    pr_number,
                ) {
                    Ok(probe) => {
                        comments = probe
                            .values
                            .iter()
                            .filter_map(review_comment_record)
                            .map(|mut comment| {
                                comment.path =
                                    comment.path.take().map(|value| normalize_path(&value));
                                comment
                            })
                            .collect();
                        normalize_review_comment_thread_ids(&mut comments);
                        if let Some(error) = probe.reason {
                            partial_sources.push(String::from("review_comments"));
                            merge_probe_reason(
                                &mut status,
                                &mut reason,
                                &mut warnings,
                                Some(error),
                                Status::Partial,
                            );
                        }
                    }
                    Err(error) => {
                        partial_sources.push(String::from("review_comments"));
                        merge_probe_reason(
                            &mut status,
                            &mut reason,
                            &mut warnings,
                            Some(error),
                            Status::Partial,
                        );
                    }
                }

                match gh::issue_comments(
                    &repo_root.path,
                    &repository.full_name,
                    repository.host.as_str(),
                    pr_number,
                ) {
                    Ok(probe) => {
                        issue_comments = probe
                            .values
                            .iter()
                            .filter_map(issue_comment_record)
                            .collect();
                        normalize_issue_comment_thread_ids(&mut issue_comments);
                        if let Some(error) = probe.reason {
                            partial_sources.push(String::from("issue_comments"));
                            merge_probe_reason(
                                &mut status,
                                &mut reason,
                                &mut warnings,
                                Some(error),
                                Status::Partial,
                            );
                        }
                    }
                    Err(error) => {
                        partial_sources.push(String::from("issue_comments"));
                        merge_probe_reason(
                            &mut status,
                            &mut reason,
                            &mut warnings,
                            Some(error),
                            Status::Partial,
                        );
                    }
                }

                review_threads = build_conversation_threads_for_author(
                    &comments,
                    &issue_comments,
                    Some(pr.author.as_str()),
                );
            } else if pr.number.is_some() && repository.identity.is_none() {
                partial_sources.push(String::from("repository_identity"));
                merge_probe_reason(
                    &mut status,
                    &mut reason,
                    &mut warnings,
                    repository.reason.clone().or_else(|| {
                        Some(String::from(
                            "Could not parse GitHub repository identity from git remotes",
                        ))
                    }),
                    Status::Partial,
                );
            }
        }
        Err(error) => {
            merge_probe_reason(
                &mut status,
                &mut reason,
                &mut warnings,
                Some(error),
                Status::Partial,
            );
            let fallback_base_branch = git::fallback_base_branch(&repo_root.path);
            let base_branch = non_empty_base_branch(&fallback_base_branch.value);
            let local_changed = git::changed_files(&repo_root.path, base_branch);
            changed_files = local_changed.paths;
            merge_probe_reason(
                &mut status,
                &mut reason,
                &mut warnings,
                fallback_base_branch.reason,
                Status::Partial,
            );
            merge_probe_reason(
                &mut status,
                &mut reason,
                &mut warnings,
                local_changed.reason,
                Status::Partial,
            );
            partial_sources.push(String::from("gh_pr_view"));
        }
    }

    changed_files = changed_files
        .into_iter()
        .map(|path| normalize_path(&path))
        .collect::<Vec<_>>();

    let artifact = ScanArtifact {
        status,
        reason: reason.clone(),
        scan_partial: status != Status::Ok,
        repo_root: Some(repo_root.path.to_string_lossy().into_owned()),
        branch: Some(branch.value),
        pr,
        files_changed: changed_files.len(),
        review_comments: comments.len(),
        threads: review_threads.len(),
        codeowners_found: codeowners_file.found,
        policy_found: policy.found,
        product_boundary: ProductBoundarySnapshot::default(),
        changed_files,
        comments,
        issue_comments,
        review_threads,
        partial_sources,
        warnings,
    };

    artifacts::write_json(run.directory.join("scan.json"), &artifact).map_err(io_error)?;
    run_store::write_latest(&run).map_err(io_error)?;

    Ok(CommandOutcome {
        status: artifact.status,
        reason,
        lines: vec![
            format!(
                "PR: {}",
                artifact
                    .pr
                    .number
                    .map(|value| format!("#{value}"))
                    .unwrap_or_else(|| String::from("unknown"))
            ),
            format!("Changed files: {}", artifact.files_changed),
            format!("Review comments: {}", artifact.review_comments),
            format!("Threads: {}", artifact.threads),
            format!("Codeowners found: {}", yes_no(artifact.codeowners_found)),
            format!("Policy file found: {}", yes_no(artifact.policy_found)),
        ],
        next: artifact
            .scan_partial
            .then(|| String::from("scan_partial=true")),
    })
}

fn repository_identity_from_pr_url(value: &Value) -> Option<git::RepositoryIdentity> {
    parse_pr_url_repository_identity(value.get("url").and_then(Value::as_str)?)
}

fn parse_pr_url_repository_identity(url: &str) -> Option<git::RepositoryIdentity> {
    let (_, remainder) = url.split_once("://")?;
    let (authority, path) = remainder.split_once('/')?;
    let host = authority.split('@').next_back()?.split(':').next()?;
    let mut parts = path.split('/').filter(|part| !part.is_empty());
    let owner = parts.next()?;
    let name = parts.next()?;
    let marker = parts.next()?;
    if host.is_empty() || owner.is_empty() || name.is_empty() || marker != "pull" {
        return None;
    }

    Some(git::RepositoryIdentity {
        host: host.to_owned(),
        full_name: format!("{owner}/{name}"),
    })
}

#[cfg(test)]
#[allow(dead_code)]
pub fn parse_pr_url_repository_identity_for_tests(url: &str) -> Option<git::RepositoryIdentity> {
    parse_pr_url_repository_identity(url)
}

fn build_pull_request_summary(value: &Value) -> PullRequestSummary {
    let mut review_decisions = value
        .get("reviewDecision")
        .and_then(Value::as_str)
        .filter(|decision| !decision.trim().is_empty())
        .map(|decision| vec![decision.to_owned()])
        .unwrap_or_default();

    for review_state in value
        .get("reviews")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|review| review.get("state").and_then(Value::as_str))
        .filter(|state| !state.trim().is_empty())
    {
        if !review_decisions
            .iter()
            .any(|existing| existing == review_state)
        {
            review_decisions.push(review_state.to_owned());
        }
    }

    PullRequestSummary {
        number: value.get("number").and_then(Value::as_u64),
        title: value
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        body: value
            .get("body")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        author: value
            .get("author")
            .and_then(|author| author.get("login"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        base_branch: value
            .get("baseRefName")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        head_branch: value
            .get("headRefName")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        labels: value
            .get("labels")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|label| label.get("name").and_then(Value::as_str))
            .map(ToOwned::to_owned)
            .collect(),
        review_decisions,
        url: value
            .get("url")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
    }
}

#[cfg(test)]
#[allow(dead_code)]
pub fn build_pull_request_summary_for_tests(value: &Value) -> PullRequestSummary {
    build_pull_request_summary(value)
}

fn pr_changed_files(value: &Value) -> Vec<String> {
    value
        .get("files")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|file| file.get("path").and_then(Value::as_str))
        .map(normalize_path)
        .collect()
}

fn api_changed_files(values: &[Value]) -> Vec<String> {
    values
        .iter()
        .filter_map(|file| {
            file.get("filename")
                .or_else(|| file.get("path"))
                .and_then(Value::as_str)
        })
        .map(normalize_path)
        .collect()
}

fn merge_changed_files(mut primary: Vec<String>, supplemental: &[String]) -> Vec<String> {
    for path in supplemental {
        if !primary.iter().any(|existing| existing == path) {
            primary.push(path.clone());
        }
    }
    primary
}

#[cfg(test)]
#[allow(dead_code)]
pub fn merge_changed_files_for_tests(primary: Vec<String>, supplemental: &[String]) -> Vec<String> {
    merge_changed_files(primary, supplemental)
}

#[cfg(test)]
#[allow(dead_code)]
pub fn api_changed_files_for_tests(values: &[Value]) -> Vec<String> {
    api_changed_files(values)
}

fn review_comment_record(value: &Value) -> Option<CommentRecord> {
    let id = value.get("id")?.as_u64()?.to_string();
    Some(CommentRecord {
        comment_id: id.clone(),
        thread_id: id,
        author: value
            .get("user")
            .and_then(|user| user.get("login"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        body: value
            .get("body")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        path: value
            .get("path")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        source: CommentSource::ReviewComment,
        reply_to_comment_id: value
            .get("in_reply_to_id")
            .and_then(Value::as_u64)
            .map(|number| number.to_string()),
        created_at: value
            .get("created_at")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        line: value.get("line").and_then(Value::as_u64),
        original_line: value.get("original_line").and_then(Value::as_u64),
    })
}

#[cfg(test)]
#[allow(dead_code)]
pub fn review_comment_record_for_tests(value: &Value) -> Option<CommentRecord> {
    review_comment_record(value)
}

fn normalize_review_comment_thread_ids(comments: &mut [CommentRecord]) {
    let by_id = comments
        .iter()
        .map(|comment| {
            (
                comment.comment_id.clone(),
                comment.reply_to_comment_id.clone(),
            )
        })
        .collect::<HashMap<_, _>>();
    for comment in comments {
        comment.thread_id = root_comment_id(comment.comment_id.as_str(), &by_id);
    }
}

fn root_comment_id(comment_id: &str, by_id: &HashMap<String, Option<String>>) -> String {
    let mut current = comment_id;
    let mut visited = Vec::<String>::new();
    while let Some(Some(parent_id)) = by_id.get(current) {
        if visited.iter().any(|seen| seen == parent_id) {
            break;
        }
        visited.push(parent_id.clone());
        current = parent_id;
    }
    current.to_owned()
}

fn normalize_issue_comment_thread_ids(comments: &mut [CommentRecord]) {
    for comment in comments {
        let thread_id = comment.thread_id.trim();
        let per_comment_thread_id = format!("issue:{}", comment.comment_id);
        if thread_id.is_empty()
            || thread_id == comment.comment_id
            || thread_id == per_comment_thread_id
        {
            comment.thread_id = ISSUE_CONVERSATION_THREAD_ID.to_owned();
        } else if !thread_id.starts_with("issue:") {
            comment.thread_id = format!("issue:{thread_id}");
        } else {
            comment.thread_id = thread_id.to_owned();
        }
    }
}

#[cfg(test)]
#[allow(dead_code)]
pub fn normalize_review_comment_thread_ids_for_tests(comments: &mut [CommentRecord]) {
    normalize_review_comment_thread_ids(comments);
}

#[cfg(test)]
#[allow(dead_code)]
pub fn normalize_issue_comment_thread_ids_for_tests(comments: &mut [CommentRecord]) {
    normalize_issue_comment_thread_ids(comments);
}

fn issue_comment_record(value: &Value) -> Option<CommentRecord> {
    let id = value.get("id")?.as_u64()?.to_string();
    Some(CommentRecord {
        comment_id: id.clone(),
        thread_id: ISSUE_CONVERSATION_THREAD_ID.to_owned(),
        author: value
            .get("user")
            .and_then(|user| user.get("login"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        body: value
            .get("body")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        path: None,
        source: CommentSource::IssueComment,
        reply_to_comment_id: None,
        created_at: value
            .get("created_at")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        line: None,
        original_line: None,
    })
}

fn merge_probe_reason(
    status: &mut Status,
    reason: &mut Option<String>,
    warnings: &mut Vec<String>,
    probe_reason: Option<String>,
    probe_status: Status,
) {
    if let Some(probe_reason) = probe_reason {
        *status = status.merge(probe_status);
        if reason.is_none() {
            *reason = Some(probe_reason.clone());
        }
        warnings.push(probe_reason);
    }
}

fn io_error(error: std::io::Error) -> String {
    error.to_string()
}

fn non_empty_base_branch(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then_some(trimmed)
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}
