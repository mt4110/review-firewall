use std::path::Path;

use rf_core::domain::SourceFailureReason;
use serde_json::Value;

use super::run_process;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub reason: Option<SourceFailureReason>,
    pub detail: String,
}

impl Failure {
    fn new(reason: Option<SourceFailureReason>, detail: impl Into<String>) -> Self {
        Self {
            reason,
            detail: detail.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ValuesProbe {
    pub values: Vec<Value>,
    pub failure: Option<Failure>,
}

pub fn pr_view(repo_root: &Path, pr_number: Option<u64>) -> Result<Value, Failure> {
    let fields = [
        "number",
        "title",
        "body",
        "author",
        "baseRefName",
        "headRefName",
        "headRefOid",
        "labels",
        "reviewDecision",
        "comments",
        "files",
        "reviews",
        "url",
    ]
    .join(",");

    let mut args = vec![String::from("pr"), String::from("view")];
    if let Some(pr_number) = pr_number {
        args.push(pr_number.to_string());
    }
    args.push(String::from("--json"));
    args.push(fields);

    parse_json_output(repo_root, &args, "gh pr view failed")
}

pub fn review_comments(
    repo_root: &Path,
    repository_full_name: &str,
    repository_host: &str,
    pr_number: u64,
) -> Result<ValuesProbe, Failure> {
    paged_array(
        repo_root,
        repository_full_name,
        repository_host,
        pr_number,
        "pulls",
        "comments",
        "gh review comments failed",
    )
}

pub fn issue_comments(
    repo_root: &Path,
    repository_full_name: &str,
    repository_host: &str,
    pr_number: u64,
) -> Result<ValuesProbe, Failure> {
    paged_array(
        repo_root,
        repository_full_name,
        repository_host,
        pr_number,
        "issues",
        "comments",
        "gh issue comments failed",
    )
}

pub fn changed_files(
    repo_root: &Path,
    repository_full_name: &str,
    repository_host: &str,
    pr_number: u64,
) -> Result<ValuesProbe, Failure> {
    paged_array(
        repo_root,
        repository_full_name,
        repository_host,
        pr_number,
        "pulls",
        "files",
        "gh changed files failed",
    )
}

fn paged_array(
    repo_root: &Path,
    repository_full_name: &str,
    repository_host: &str,
    pr_number: u64,
    resource: &str,
    suffix: &str,
    fallback: &str,
) -> Result<ValuesProbe, Failure> {
    collect_paged_arrays(|page| {
        let endpoint = format!(
            "repos/{repository_full_name}/{resource}/{pr_number}/{suffix}?per_page=100&page={page}"
        );
        let mut args = vec![String::from("api")];
        let hostname = gh_hostname(repository_host);
        if hostname != "github.com" {
            args.push(String::from("--hostname"));
            args.push(hostname.to_owned());
        }
        args.push(endpoint);
        parse_json_output(repo_root, &args, fallback)
    })
}

fn gh_hostname(repository_host: &str) -> &str {
    let repository_host = repository_host.trim();
    if repository_host.starts_with('[') {
        return repository_host;
    }
    repository_host
        .split_once(':')
        .map(|(host, _)| host)
        .unwrap_or(repository_host)
}

#[cfg(test)]
#[allow(dead_code)]
pub fn gh_hostname_for_tests(repository_host: &str) -> &str {
    gh_hostname(repository_host)
}

fn collect_paged_arrays<F>(mut fetch_page: F) -> Result<ValuesProbe, Failure>
where
    F: FnMut(usize) -> Result<Value, Failure>,
{
    let mut page = 1usize;
    let mut items = Vec::new();

    loop {
        let output = match fetch_page(page) {
            Ok(output) => output,
            Err(error) if items.is_empty() => return Err(error),
            Err(error) => {
                return Ok(ValuesProbe {
                    values: items,
                    failure: Some(pagination_failure(page - 1, error)),
                });
            }
        };
        let page_items = match output.as_array().cloned() {
            Some(page_items) => page_items,
            None if items.is_empty() => {
                return Err(Failure::new(
                    Some(SourceFailureReason::JsonParseError),
                    "gh api returned invalid JSON array",
                ));
            }
            None => {
                return Ok(ValuesProbe {
                    values: items,
                    failure: Some(Failure::new(
                        Some(SourceFailureReason::PaginationPartial),
                        format!(
                            "GitHub pagination stopped after page {}: gh api returned invalid JSON array",
                            page - 1
                        ),
                    )),
                });
            }
        };
        let page_len = page_items.len();
        items.extend(page_items);
        if page_len < 100 {
            return Ok(ValuesProbe {
                values: items,
                failure: None,
            });
        }
        page += 1;
    }
}

fn pagination_failure(last_completed_page: usize, error: Failure) -> Failure {
    Failure::new(
        Some(SourceFailureReason::PaginationPartial),
        format!(
            "GitHub pagination stopped after page {last_completed_page}: {}",
            error.detail
        ),
    )
}

#[cfg(test)]
#[allow(dead_code)]
pub fn collect_paged_arrays_for_tests(
    pages: Vec<Result<Value, Failure>>,
) -> Result<ValuesProbe, Failure> {
    let mut pages = pages.into_iter();
    collect_paged_arrays(|_| pages.next().unwrap_or_else(|| Ok(Value::Array(Vec::new()))))
}

fn parse_json_output(repo_root: &Path, args: &[String], fallback: &str) -> Result<Value, Failure> {
    let output = run_process(repo_root, "gh", args);
    if !output.success {
        let detail = output.reason.unwrap_or_else(|| {
            let trimmed = output.stderr.trim();
            if trimmed.is_empty() {
                fallback.to_owned()
            } else {
                trimmed.to_owned()
            }
        });
        return Err(Failure::new(normalize_gh_failure(&detail), detail));
    }
    serde_json::from_str(&output.stdout)
        .map_err(|error| Failure::new(Some(SourceFailureReason::JsonParseError), error.to_string()))
}

fn normalize_gh_failure(detail: &str) -> Option<SourceFailureReason> {
    let lower = detail.to_ascii_lowercase();

    if lower.contains("no such file or directory")
        || lower.contains("not found in path")
        || lower.contains("cannot find the file")
        || lower.contains("program not found")
    {
        return Some(SourceFailureReason::GhMissing);
    }

    if lower.contains("gh auth login")
        || lower.contains("not logged into")
        || lower.contains("authentication failed")
        || lower.contains("no oauth token")
        || lower.contains("token is required")
        || lower.contains("bad credentials")
        || lower.contains("http 401")
        || lower.contains("status code 401")
        || lower.contains("401 unauthorized")
        || lower.contains("requires authentication")
    {
        return Some(SourceFailureReason::GhNotAuthenticated);
    }

    if lower.contains("rate limit") {
        return Some(SourceFailureReason::GhRateLimited);
    }

    if lower.contains("pull request not found")
        || lower.contains("no pull requests found")
        || lower.contains("could not resolve to a pullrequest")
    {
        return Some(SourceFailureReason::PrNotFound);
    }

    if lower.contains("403")
        || lower.contains("forbidden")
        || lower.contains("permission denied")
        || lower.contains("resource not accessible")
        || lower.contains("insufficient_scopes")
    {
        return Some(SourceFailureReason::GhPermissionDenied);
    }

    if lower.contains("timeout")
        || lower.contains("temporarily unavailable")
        || lower.contains("connection reset")
        || lower.contains("connection refused")
        || lower.contains("connection aborted")
        || lower.contains("tls")
        || lower.contains("network")
        || lower.contains("dial tcp")
        || lower.contains("no such host")
    {
        return Some(SourceFailureReason::NetworkError);
    }

    None
}

#[cfg(test)]
#[allow(dead_code)]
pub fn normalize_gh_failure_for_tests(detail: &str) -> Option<SourceFailureReason> {
    normalize_gh_failure(detail)
}
