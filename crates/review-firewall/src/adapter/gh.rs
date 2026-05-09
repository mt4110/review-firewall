use std::path::Path;

use serde_json::Value;

use super::run_process;

#[derive(Debug, Clone)]
pub struct ValuesProbe {
    pub values: Vec<Value>,
    pub reason: Option<String>,
}

pub fn pr_view(repo_root: &Path, pr_number: Option<u64>) -> Result<Value, String> {
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
) -> Result<ValuesProbe, String> {
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
) -> Result<ValuesProbe, String> {
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
) -> Result<ValuesProbe, String> {
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
) -> Result<ValuesProbe, String> {
    collect_paged_arrays(|page| {
        let endpoint = format!(
            "repos/{repository_full_name}/{resource}/{pr_number}/{suffix}?per_page=100&page={page}"
        );
        let mut args = vec![String::from("api")];
        if repository_host != "github.com" {
            args.push(String::from("--hostname"));
            args.push(repository_host.to_owned());
        }
        args.push(endpoint);
        parse_json_output(repo_root, &args, fallback)
    })
}

fn collect_paged_arrays<F>(mut fetch_page: F) -> Result<ValuesProbe, String>
where
    F: FnMut(usize) -> Result<Value, String>,
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
                    reason: Some(error),
                });
            }
        };
        let page_items = match output.as_array().cloned() {
            Some(page_items) => page_items,
            None if items.is_empty() => {
                return Err(String::from("gh api returned invalid JSON array"));
            }
            None => {
                return Ok(ValuesProbe {
                    values: items,
                    reason: Some(String::from("gh api returned invalid JSON array")),
                });
            }
        };
        let page_len = page_items.len();
        items.extend(page_items);
        if page_len < 100 {
            return Ok(ValuesProbe {
                values: items,
                reason: None,
            });
        }
        page += 1;
    }
}

#[cfg(test)]
#[allow(dead_code)]
pub fn collect_paged_arrays_for_tests(
    pages: Vec<Result<Value, String>>,
) -> Result<ValuesProbe, String> {
    let mut pages = pages.into_iter();
    collect_paged_arrays(|_| pages.next().unwrap_or_else(|| Ok(Value::Array(Vec::new()))))
}

fn parse_json_output(repo_root: &Path, args: &[String], fallback: &str) -> Result<Value, String> {
    let output = run_process(repo_root, "gh", args);
    if !output.success {
        return Err(output.reason.unwrap_or_else(|| {
            let trimmed = output.stderr.trim();
            if trimmed.is_empty() {
                fallback.to_owned()
            } else {
                trimmed.to_owned()
            }
        }));
    }
    serde_json::from_str(&output.stdout).map_err(|error| error.to_string())
}
