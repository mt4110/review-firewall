use std::collections::HashMap;
use std::path::Path;

use rf_core::domain::{
    CommentRecord, CommentSource, DataCoverage, ProductBoundarySnapshot, PullRequestSummary,
    ReviewSignal, ScanArtifact, SourceCoverageArtifact, SourceCoverageEntry, SourceCoverageName,
    SourceCoverageStatus, SourceFailureReason, Status, derive_data_coverage_from_sources,
};
use rf_core::{build_conversation_threads_for_author, normalize_path};
use serde_json::Value;

use crate::adapter::{gh, git};
use crate::command::CommandOutcome;
use crate::io::{artifacts, codeowners, config, run_store};

struct BoolProbe {
    value: bool,
    reason: Option<String>,
    failure_reason: Option<SourceFailureReason>,
}

pub fn run(cwd: &Path, pr_override: Option<u64>) -> Result<CommandOutcome, String> {
    let repo_root = git::repo_root(cwd);
    let run = run_store::create_new(&repo_root.path).map_err(io_error)?;
    let policy = config::load(&repo_root.path);
    let codeowners_file = codeowners::load(&repo_root.path);
    let branch = git::current_branch(&repo_root.path);

    let mut status = Status::Ok;
    let mut reason = None::<String>;
    let mut warnings = Vec::new();

    merge_probe_reason(
        &mut status,
        &mut reason,
        &mut warnings,
        repo_root.reason.clone(),
        Status::Partial,
    );
    merge_probe_reason(
        &mut status,
        &mut reason,
        &mut warnings,
        branch.reason.clone(),
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

    let repo_root_source = SourceCoverageEntry::new(
        SourceCoverageName::RepoRoot,
        true,
        if repo_root.reason.is_some() {
            SourceCoverageStatus::Partial
        } else {
            SourceCoverageStatus::Full
        },
        usize::from(repo_root.reason.is_none()),
        repo_root
            .reason
            .as_ref()
            .map(|_| SourceFailureReason::LocalGitUnavailable),
        repo_root.reason.clone(),
    );
    let current_branch_source = SourceCoverageEntry::new(
        SourceCoverageName::CurrentBranch,
        false,
        if branch.reason.is_some() {
            SourceCoverageStatus::Partial
        } else {
            SourceCoverageStatus::Full
        },
        usize::from(branch.reason.is_none()),
        branch
            .reason
            .as_ref()
            .map(|_| SourceFailureReason::LocalGitUnavailable),
        branch.reason.clone(),
    );
    let config_source = SourceCoverageEntry::new(
        SourceCoverageName::Config,
        false,
        if policy.reason.is_some() {
            SourceCoverageStatus::Partial
        } else {
            SourceCoverageStatus::Full
        },
        usize::from(policy.found),
        None,
        policy.reason.clone(),
    );
    let codeowners_source = SourceCoverageEntry::new(
        SourceCoverageName::Codeowners,
        false,
        if codeowners_file.reason.is_some() {
            SourceCoverageStatus::Partial
        } else {
            SourceCoverageStatus::Full
        },
        usize::from(codeowners_file.found),
        None,
        codeowners_file.reason.clone(),
    );

    let pr_view = gh::pr_view(&repo_root.path, pr_override);
    let mut pr = PullRequestSummary::default();
    let mut changed_files;
    let mut comments = Vec::<CommentRecord>::new();
    let mut issue_comments = Vec::<CommentRecord>::new();
    let mut review_threads = Vec::new();
    let pr_metadata_source;
    let mut changed_files_source = skipped_source(SourceCoverageName::ChangedFiles, true);
    let mut review_comments_source = skipped_source(SourceCoverageName::ReviewComments, true);
    let review_body_comments_source;
    let mut issue_comments_source = skipped_source(SourceCoverageName::IssueComments, true);
    let review_decision_source;

    match pr_view {
        Ok(pr_data) => {
            pr = build_pull_request_summary(&pr_data);
            changed_files = pr_changed_files(&pr_data);
            let pr_view_changed_files_seen = changed_files.len();
            comments = review_body_comment_records(&pr_data);
            let review_body_comments_seen = comments.len();
            issue_comments = pr_view_issue_comment_records(&pr_data);
            normalize_issue_comment_thread_ids(&mut issue_comments);
            let pr_view_issue_comments_seen = issue_comments.len();
            pr_metadata_source = SourceCoverageEntry::new(
                SourceCoverageName::PrMetadata,
                true,
                SourceCoverageStatus::Full,
                1,
                None,
                None,
            );
            review_body_comments_source = SourceCoverageEntry::new(
                SourceCoverageName::ReviewBodyComments,
                true,
                SourceCoverageStatus::Full,
                review_body_comments_seen,
                None,
                None,
            );
            review_decision_source = SourceCoverageEntry::new(
                SourceCoverageName::ReviewDecision,
                false,
                SourceCoverageStatus::Full,
                pr.review_decisions.len(),
                None,
                None,
            );
            let repository = repository_identity_from_pr_url(&pr_data)
                .map(|identity| git::RepositoryProbe {
                    identity: Some(identity),
                    reason: None,
                })
                .unwrap_or_else(|| git::repository_identity(&repo_root.path));

            if let (Some(pr_number), Some(repository)) = (pr.number, repository.identity.clone()) {
                let mut changed_files_partial = false;
                let mut changed_files_failure = None::<(Option<SourceFailureReason>, String)>;
                let mut changed_files_detail = None::<String>;
                let mut used_local_changed_file_supplement = false;

                match gh::changed_files(
                    &repo_root.path,
                    &repository.full_name,
                    repository.host.as_str(),
                    pr_number,
                ) {
                    Ok(probe) => {
                        changed_files =
                            merge_changed_files(changed_files, &api_changed_files(&probe.values));
                        if let Some(error) = probe.failure {
                            changed_files_partial = true;
                            merge_probe_reason(
                                &mut status,
                                &mut reason,
                                &mut warnings,
                                Some(error.detail.clone()),
                                Status::Partial,
                            );
                            changed_files_failure = Some((error.reason, error.detail.clone()));
                        }
                    }
                    Err(error) => {
                        changed_files_partial = true;
                        merge_probe_reason(
                            &mut status,
                            &mut reason,
                            &mut warnings,
                            Some(error.detail.clone()),
                            Status::Partial,
                        );
                        changed_files_failure = Some((error.reason, error.detail.clone()));
                    }
                }

                if changed_files.is_empty() || changed_files_partial {
                    let supplement = verified_base_changed_files(
                        &repo_root.path,
                        pr.base_branch.as_deref(),
                        pr.head_oid.as_deref(),
                    );
                    if let Some(probe_reason) = supplement.reason.clone() {
                        merge_probe_reason(
                            &mut status,
                            &mut reason,
                            &mut warnings,
                            Some(probe_reason.clone()),
                            Status::Partial,
                        );
                        if changed_files_failure.is_none() {
                            changed_files_failure =
                                Some((supplement.failure_reason, probe_reason.clone()));
                        }
                    }
                    used_local_changed_file_supplement = !supplement.paths.is_empty();
                    changed_files = merge_changed_files(changed_files, &supplement.paths);
                    if used_local_changed_file_supplement {
                        changed_files_detail = Some(String::from(
                            "Changed files were supplemented from a verified local base diff because GitHub coverage was incomplete.",
                        ));
                    }
                }
                let changed_files_status = if changed_files_failure.is_none()
                    && !changed_files_partial
                    && !used_local_changed_file_supplement
                {
                    SourceCoverageStatus::Full
                } else {
                    coverage_status_for_observed_items(changed_files.len())
                };
                let (changed_files_reason, changed_files_reason_detail) =
                    changed_files_failure.unwrap_or((None, String::new()));
                changed_files_source = SourceCoverageEntry::new(
                    SourceCoverageName::ChangedFiles,
                    true,
                    changed_files_status,
                    changed_files.len(),
                    changed_files_reason,
                    changed_files_detail.or_else(|| {
                        (!changed_files_reason_detail.is_empty())
                            .then_some(changed_files_reason_detail)
                    }),
                );

                let mut line_review_comments_seen = 0usize;
                let mut review_comments_failure = None::<(Option<SourceFailureReason>, String)>;
                match gh::review_comments(
                    &repo_root.path,
                    &repository.full_name,
                    repository.host.as_str(),
                    pr_number,
                ) {
                    Ok(probe) => {
                        let review_comment_records = probe
                            .values
                            .iter()
                            .filter_map(review_comment_record)
                            .map(|mut comment| {
                                comment.path =
                                    comment.path.take().map(|value| normalize_path(&value));
                                comment
                            })
                            .collect::<Vec<_>>();
                        line_review_comments_seen = review_comment_records.len();
                        comments.extend(review_comment_records);
                        if let Some(error) = probe.failure {
                            merge_probe_reason(
                                &mut status,
                                &mut reason,
                                &mut warnings,
                                Some(error.detail.clone()),
                                Status::Partial,
                            );
                            review_comments_failure = Some((error.reason, error.detail.clone()));
                        }
                    }
                    Err(error) => {
                        merge_probe_reason(
                            &mut status,
                            &mut reason,
                            &mut warnings,
                            Some(error.detail.clone()),
                            Status::Partial,
                        );
                        review_comments_failure = Some((error.reason, error.detail.clone()));
                    }
                }
                let review_comments_status = if review_comments_failure.is_none() {
                    SourceCoverageStatus::Full
                } else {
                    coverage_status_for_observed_items(line_review_comments_seen)
                };
                let (review_comments_reason, review_comments_detail) =
                    review_comments_failure.unwrap_or((None, String::new()));
                review_comments_source = SourceCoverageEntry::new(
                    SourceCoverageName::ReviewComments,
                    true,
                    review_comments_status,
                    line_review_comments_seen,
                    review_comments_reason,
                    (!review_comments_detail.is_empty()).then_some(review_comments_detail),
                );
                normalize_review_comment_thread_ids(&mut comments);

                let mut api_issue_comments_seen = 0usize;
                let mut issue_comments_failure = None::<(Option<SourceFailureReason>, String)>;
                match gh::issue_comments(
                    &repo_root.path,
                    &repository.full_name,
                    repository.host.as_str(),
                    pr_number,
                ) {
                    Ok(probe) => {
                        let api_issue_comments = probe
                            .values
                            .iter()
                            .filter_map(issue_comment_record)
                            .collect::<Vec<_>>();
                        api_issue_comments_seen = api_issue_comments.len();
                        issue_comments = api_issue_comments;
                        normalize_issue_comment_thread_ids(&mut issue_comments);
                        if let Some(error) = probe.failure {
                            merge_probe_reason(
                                &mut status,
                                &mut reason,
                                &mut warnings,
                                Some(error.detail.clone()),
                                Status::Partial,
                            );
                            issue_comments_failure = Some((error.reason, error.detail.clone()));
                        }
                    }
                    Err(error) => {
                        merge_probe_reason(
                            &mut status,
                            &mut reason,
                            &mut warnings,
                            Some(error.detail.clone()),
                            Status::Partial,
                        );
                        issue_comments_failure = Some((error.reason, error.detail.clone()));
                    }
                }
                let issue_comment_items_seen = if issue_comments_failure.is_some() {
                    issue_comments.len().max(pr_view_issue_comments_seen)
                } else {
                    api_issue_comments_seen
                };
                let issue_comments_status = if issue_comments_failure.is_none() {
                    SourceCoverageStatus::Full
                } else {
                    coverage_status_for_observed_items(issue_comment_items_seen)
                };
                let (issue_comments_reason, issue_comments_detail) =
                    issue_comments_failure.unwrap_or((None, String::new()));
                issue_comments_source = SourceCoverageEntry::new(
                    SourceCoverageName::IssueComments,
                    true,
                    issue_comments_status,
                    issue_comment_items_seen,
                    issue_comments_reason,
                    (!issue_comments_detail.is_empty()).then_some(issue_comments_detail),
                );

                review_threads = build_conversation_threads_for_author(
                    &comments,
                    &issue_comments,
                    Some(pr.author.as_str()),
                );
            } else if pr.number.is_some() && repository.identity.is_none() {
                let repository_reason = repository.reason.clone().unwrap_or_else(|| {
                    String::from("Could not parse GitHub repository identity from git remotes")
                });
                let repository_failure_reason =
                    Some(normalize_repository_identity_failure(&repository_reason));

                if changed_files.is_empty() {
                    let supplement = verified_base_changed_files(
                        &repo_root.path,
                        pr.base_branch.as_deref(),
                        pr.head_oid.as_deref(),
                    );
                    if let Some(probe_reason) = supplement.reason.clone() {
                        merge_probe_reason(
                            &mut status,
                            &mut reason,
                            &mut warnings,
                            Some(probe_reason),
                            Status::Partial,
                        );
                    }
                    changed_files = merge_changed_files(changed_files, &supplement.paths);
                }
                merge_probe_reason(
                    &mut status,
                    &mut reason,
                    &mut warnings,
                    Some(repository_reason.clone()),
                    Status::Partial,
                );
                changed_files_source = SourceCoverageEntry::new(
                    SourceCoverageName::ChangedFiles,
                    true,
                    coverage_status_for_observed_items(changed_files.len()),
                    changed_files.len().max(pr_view_changed_files_seen),
                    repository_failure_reason,
                    Some(repository_reason.clone()),
                );
                review_comments_source = SourceCoverageEntry::new(
                    SourceCoverageName::ReviewComments,
                    true,
                    SourceCoverageStatus::Failed,
                    0,
                    repository_failure_reason,
                    Some(repository_reason.clone()),
                );
                issue_comments_source = SourceCoverageEntry::new(
                    SourceCoverageName::IssueComments,
                    true,
                    coverage_status_for_observed_items(pr_view_issue_comments_seen),
                    pr_view_issue_comments_seen,
                    repository_failure_reason,
                    Some(repository_reason),
                );
            }
        }
        Err(error) => {
            merge_probe_reason(
                &mut status,
                &mut reason,
                &mut warnings,
                Some(error.detail.clone()),
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
            pr_metadata_source = SourceCoverageEntry::new(
                SourceCoverageName::PrMetadata,
                true,
                SourceCoverageStatus::Failed,
                0,
                error.reason,
                Some(error.detail.clone()),
            );
            review_body_comments_source = SourceCoverageEntry::new(
                SourceCoverageName::ReviewBodyComments,
                true,
                SourceCoverageStatus::Failed,
                0,
                error.reason,
                Some(error.detail.clone()),
            );
            review_comments_source = SourceCoverageEntry::new(
                SourceCoverageName::ReviewComments,
                true,
                SourceCoverageStatus::Failed,
                0,
                error.reason,
                Some(error.detail.clone()),
            );
            issue_comments_source = SourceCoverageEntry::new(
                SourceCoverageName::IssueComments,
                true,
                SourceCoverageStatus::Failed,
                0,
                error.reason,
                Some(error.detail.clone()),
            );
            review_decision_source = SourceCoverageEntry::new(
                SourceCoverageName::ReviewDecision,
                false,
                SourceCoverageStatus::Failed,
                0,
                error.reason,
                Some(error.detail.clone()),
            );
            changed_files_source = SourceCoverageEntry::new(
                SourceCoverageName::ChangedFiles,
                true,
                coverage_status_for_observed_items(changed_files.len()),
                changed_files.len(),
                error.reason,
                Some(String::from(
                    "Changed files fell back to local git because PR metadata could not be fully observed from GitHub.",
                )),
            );
        }
    }

    if review_threads.is_empty() && (!comments.is_empty() || !issue_comments.is_empty()) {
        review_threads = build_conversation_threads_for_author(
            &comments,
            &issue_comments,
            Some(pr.author.as_str()),
        );
    }

    changed_files = changed_files
        .into_iter()
        .map(|path| normalize_path(&path))
        .collect::<Vec<_>>();
    let sources = vec![
        repo_root_source,
        current_branch_source,
        config_source,
        codeowners_source,
        pr_metadata_source,
        changed_files_source,
        review_comments_source,
        review_body_comments_source,
        issue_comments_source,
        review_decision_source,
    ];
    let data_coverage = derive_data_coverage_from_sources(&sources);
    let partial_sources = sources
        .iter()
        .filter(|source| source.required && source.status != SourceCoverageStatus::Full)
        .map(|source| source.name.as_str().to_owned())
        .collect::<Vec<_>>();
    let source_coverage_artifact = SourceCoverageArtifact {
        status,
        data_coverage,
        review_signal: ReviewSignal::Unknown,
        reason: reason.clone(),
        sources,
        warnings: warnings.clone(),
    };

    let artifact = ScanArtifact {
        status,
        data_coverage,
        review_signal: ReviewSignal::Unknown,
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

    artifacts::write_json(
        run.directory.join("source_coverage.json"),
        &source_coverage_artifact,
    )
    .map_err(io_error)?;
    artifacts::write_json(run.directory.join("scan.json"), &artifact).map_err(io_error)?;
    run_store::write_latest(&run).map_err(io_error)?;

    Ok(CommandOutcome {
        status: artifact.status,
        data_coverage: artifact.data_coverage,
        review_signal: artifact.review_signal,
        residual_blockers: 0,
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
        next: if artifact.data_coverage != DataCoverage::Full {
            Some(String::from(
                "Inspect source_coverage.json for missing review inputs, then rerun review-firewall scan.",
            ))
        } else {
            artifact
                .scan_partial
                .then(|| String::from("scan completed with partial local metadata"))
        },
    })
}

fn repository_identity_from_pr_url(value: &Value) -> Option<git::RepositoryIdentity> {
    parse_pr_url_repository_identity(value.get("url").and_then(Value::as_str)?)
}

fn parse_pr_url_repository_identity(url: &str) -> Option<git::RepositoryIdentity> {
    let (_, remainder) = url.split_once("://")?;
    let (authority, path) = remainder.split_once('/')?;
    let authority_host = authority.rsplit('@').next().unwrap_or(authority);
    let host = authority_host_without_port(authority_host);
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

fn authority_host_without_port(host: &str) -> &str {
    if host.starts_with('[') {
        return host;
    }
    host.split_once(':').map(|(host, _)| host).unwrap_or(host)
}

#[cfg(test)]
#[allow(dead_code)]
pub fn parse_pr_url_repository_identity_for_tests(url: &str) -> Option<git::RepositoryIdentity> {
    parse_pr_url_repository_identity(url)
}

fn build_pull_request_summary(value: &Value) -> PullRequestSummary {
    let review_decisions = value
        .get("reviewDecision")
        .and_then(Value::as_str)
        .filter(|decision| !decision.trim().is_empty())
        .map(|decision| vec![decision.to_owned()])
        .unwrap_or_else(|| fallback_review_decisions(value));

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
        head_oid: value
            .get("headRefOid")
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

fn fallback_review_decisions(value: &Value) -> Vec<String> {
    let Some(reviews) = value.get("reviews").and_then(Value::as_array) else {
        return Vec::new();
    };

    let mut ordered_reviews = reviews
        .iter()
        .enumerate()
        .filter_map(|(index, review)| {
            let state = review.get("state").and_then(Value::as_str)?.trim();
            if state.is_empty() {
                return None;
            }
            Some((
                review
                    .get("submittedAt")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                index,
                state.to_owned(),
            ))
        })
        .collect::<Vec<_>>();

    ordered_reviews.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));

    let mut last_decisive_state = None::<String>;
    let mut last_commented_state = None::<String>;

    for (_, _, state) in ordered_reviews {
        match state.as_str() {
            "APPROVED" | "CHANGES_REQUESTED" => last_decisive_state = Some(state),
            "DISMISSED" => last_decisive_state = None,
            "COMMENTED" => last_commented_state = Some(state),
            _ => last_decisive_state = Some(state),
        }
    }

    last_decisive_state
        .or(last_commented_state)
        .into_iter()
        .collect()
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
fn supplement_changed_files_from_base_diff(
    primary: Vec<String>,
    base_diff: Vec<String>,
    local_head_matches_pr_head: bool,
) -> Vec<String> {
    if local_head_matches_pr_head {
        merge_changed_files(primary, &base_diff)
    } else {
        primary
    }
}

struct ChangedFileSupplementProbe {
    paths: Vec<String>,
    reason: Option<String>,
    failure_reason: Option<SourceFailureReason>,
}

fn verified_base_changed_files(
    repo_root: &Path,
    base_branch: Option<&str>,
    pr_head_oid: Option<&str>,
) -> ChangedFileSupplementProbe {
    let head_check = local_head_matches_pr_head(repo_root, pr_head_oid);
    let head_matches = head_check.value;
    let head_reason = head_check.reason.clone();
    let head_failure_reason = head_check.failure_reason;

    if !head_matches {
        return ChangedFileSupplementProbe {
            paths: Vec::new(),
            reason: head_reason,
            failure_reason: head_failure_reason,
        };
    }

    let base_changed = git::changed_files_against_base(repo_root, base_branch);
    ChangedFileSupplementProbe {
        paths: base_changed.paths,
        reason: base_changed.reason.clone(),
        failure_reason: base_changed
            .reason
            .as_ref()
            .map(|_| SourceFailureReason::LocalGitUnavailable),
    }
}

fn local_head_matches_pr_head(repo_root: &Path, pr_head_oid: Option<&str>) -> BoolProbe {
    let Some(pr_head_oid) = pr_head_oid.map(str::trim).filter(|value| !value.is_empty()) else {
        return BoolProbe {
            value: false,
            reason: Some(String::from(
                "Skipped local changed-file supplementation because PR head OID is unknown",
            )),
            failure_reason: None,
        };
    };

    let local_head = git::head_oid(repo_root);
    if let Some(reason) = local_head.reason {
        return BoolProbe {
            value: false,
            reason: Some(reason),
            failure_reason: Some(SourceFailureReason::LocalGitUnavailable),
        };
    }

    if local_head.value.eq_ignore_ascii_case(pr_head_oid) {
        BoolProbe {
            value: true,
            reason: None,
            failure_reason: None,
        }
    } else {
        BoolProbe {
            value: false,
            reason: Some(format!(
                "Skipped local changed-file supplementation because local HEAD {} does not match PR head {}",
                short_oid(&local_head.value),
                short_oid(pr_head_oid)
            )),
            failure_reason: Some(SourceFailureReason::HeadOidMismatch),
        }
    }
}

fn short_oid(value: &str) -> String {
    value.chars().take(12).collect()
}

#[cfg(test)]
#[allow(dead_code)]
pub fn merge_changed_files_for_tests(primary: Vec<String>, supplemental: &[String]) -> Vec<String> {
    merge_changed_files(primary, supplemental)
}

#[cfg(test)]
#[allow(dead_code)]
pub fn supplement_changed_files_from_base_diff_for_tests(
    primary: Vec<String>,
    base_diff: Vec<String>,
    local_head_matches_pr_head: bool,
) -> Vec<String> {
    supplement_changed_files_from_base_diff(primary, base_diff, local_head_matches_pr_head)
}

#[cfg(test)]
#[allow(dead_code)]
pub fn api_changed_files_for_tests(values: &[Value]) -> Vec<String> {
    api_changed_files(values)
}

fn review_body_comment_records(value: &Value) -> Vec<CommentRecord> {
    value
        .get("reviews")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(review_body_comment_record)
        .collect()
}

fn pr_view_issue_comment_records(value: &Value) -> Vec<CommentRecord> {
    value
        .get("comments")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(issue_comment_record)
        .collect()
}

#[cfg(test)]
#[allow(dead_code)]
pub fn pr_view_issue_comment_records_for_tests(value: &Value) -> Vec<CommentRecord> {
    pr_view_issue_comment_records(value)
}

fn review_body_comment_record(value: &Value) -> Option<CommentRecord> {
    let body = value.get("body")?.as_str()?.trim();
    if body.is_empty() {
        return None;
    }
    let id = json_id_string(value.get("id")?)?;
    let comment_id = format!("review:{id}");
    Some(CommentRecord {
        comment_id: comment_id.clone(),
        thread_id: comment_id,
        author: value
            .get("author")
            .and_then(|author| author.get("login"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        body: body.to_owned(),
        path: None,
        source: CommentSource::ReviewComment,
        reply_to_comment_id: None,
        created_at: value
            .get("submittedAt")
            .or_else(|| value.get("submitted_at"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        line: None,
        original_line: None,
    })
}

#[cfg(test)]
#[allow(dead_code)]
pub fn review_body_comment_records_for_tests(value: &Value) -> Vec<CommentRecord> {
    review_body_comment_records(value)
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

fn json_id_string(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(ToOwned::to_owned)
        .or_else(|| value.as_u64().map(|number| number.to_string()))
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
        if !by_id.contains_key(parent_id) {
            return parent_id.to_owned();
        }
        if visited.iter().any(|seen| seen == parent_id) {
            return comment_id.to_owned();
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
            comment.thread_id = per_comment_thread_id;
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
    let id = json_id_string(value.get("id")?)?;
    Some(CommentRecord {
        comment_id: id.clone(),
        thread_id: format!("issue:{id}"),
        author: value
            .get("user")
            .or_else(|| value.get("author"))
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
            .or_else(|| value.get("createdAt"))
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

fn skipped_source(name: SourceCoverageName, required: bool) -> SourceCoverageEntry {
    SourceCoverageEntry::new(name, required, SourceCoverageStatus::Skipped, 0, None, None)
}

fn coverage_status_for_observed_items(items_seen: usize) -> SourceCoverageStatus {
    if items_seen > 0 {
        SourceCoverageStatus::Partial
    } else {
        SourceCoverageStatus::Failed
    }
}

fn normalize_repository_identity_failure(detail: &str) -> SourceFailureReason {
    if detail.trim().contains("No git remotes configured") {
        SourceFailureReason::RepositoryIdentityUnknown
    } else {
        SourceFailureReason::UnsupportedRemote
    }
}

fn non_empty_base_branch(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then_some(trimmed)
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}
