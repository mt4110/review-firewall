#![allow(dead_code)]

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[path = "../src/adapter/mod.rs"]
mod adapter;
#[path = "../src/command/mod.rs"]
mod command;
#[path = "../src/io/mod.rs"]
mod io;

use serde_json::Value;

use rf_core::domain::{CommentRecord, CommentSource};

fn temp_dir(name: &str) -> PathBuf {
    let directory = env::temp_dir().join(format!(
        "review-firewall-{name}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("unix time")
            .as_nanos()
    ));
    fs::create_dir_all(&directory).expect("temp dir");
    directory
}

fn run_git(repo: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {:?} failed\nstdout={}\nstderr={}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn config_loads_known_keys_and_ignores_unknown_ones() {
    let repo = temp_dir("config");
    fs::write(
        repo.join("review-firewall.toml"),
        "version = 1\n[review]\nmax_pr_thread_roundtrips = 4\n[blocker]\nrequire_evidence = false\n[unknown]\nvalue = true\n",
    )
    .expect("write config");

    let loaded = io::config::load(&repo);

    assert!(loaded.found);
    assert_eq!(loaded.review.max_pr_thread_roundtrips, 4);
    assert!(!loaded.blocker.require_evidence);
    assert_eq!(loaded.status.terminal_label(), "PARTIAL");
}

#[test]
fn config_marks_invalid_known_values_as_partial() {
    let repo = temp_dir("config-invalid");
    fs::write(
        repo.join("review-firewall.toml"),
        "version = 1\n[blocker]\nrequire_evidence = flase\n",
    )
    .expect("write config");

    let loaded = io::config::load(&repo);

    assert!(loaded.found);
    assert_eq!(loaded.status.terminal_label(), "PARTIAL");
    assert_eq!(
        loaded.reason.as_deref(),
        Some("Invalid config value for blocker.require_evidence: flase")
    );
}

#[test]
fn codeowners_detection_reads_rules() {
    let repo = temp_dir("codeowners");
    fs::create_dir_all(repo.join(".github")).expect("codeowners dir");
    fs::write(
        repo.join(".github").join("CODEOWNERS"),
        "/src/* @reviewer-a\n",
    )
    .expect("write codeowners");

    let loaded = io::codeowners::load(&repo);

    assert!(loaded.found);
    assert_eq!(loaded.rules.len(), 1);
}

#[test]
fn codeowners_detection_uses_root_then_docs_fallbacks() {
    let root_repo = temp_dir("codeowners-root");
    fs::write(root_repo.join("CODEOWNERS"), "/src/* @root-owner\n").expect("write root");

    let docs_repo = temp_dir("codeowners-docs");
    fs::create_dir_all(docs_repo.join("docs")).expect("docs dir");
    fs::write(
        docs_repo.join("docs").join("CODEOWNERS"),
        "/docs/* @docs-owner\n",
    )
    .expect("write docs");

    let root_loaded = io::codeowners::load(&root_repo);
    let docs_loaded = io::codeowners::load(&docs_repo);

    assert!(root_loaded.found);
    assert_eq!(
        root_loaded.rules[0].owners,
        vec![String::from("@root-owner")]
    );
    assert!(docs_loaded.found);
    assert_eq!(
        docs_loaded.rules[0].owners,
        vec![String::from("@docs-owner")]
    );
}

#[test]
fn codeowners_detection_prefers_github_directory() {
    let repo = temp_dir("codeowners-priority");
    fs::create_dir_all(repo.join(".github")).expect("github dir");
    fs::write(repo.join("CODEOWNERS"), "/src/* @root-owner\n").expect("write root");
    fs::write(
        repo.join(".github").join("CODEOWNERS"),
        "/src/* @github-owner\n",
    )
    .expect("write github");

    let loaded = io::codeowners::load(&repo);

    assert!(loaded.found);
    assert_eq!(loaded.rules[0].owners, vec![String::from("@github-owner")]);
}

#[test]
fn codeowners_detection_preserves_empty_owner_overrides() {
    let repo = temp_dir("codeowners-empty-owner");
    fs::write(repo.join("CODEOWNERS"), "/apps/ @platform\n/apps/github\n").expect("write root");

    let loaded = io::codeowners::load(&repo);

    assert!(loaded.found);
    assert_eq!(loaded.rules.len(), 2);
    assert_eq!(loaded.rules[1].pattern, "/apps/github");
    assert!(loaded.rules[1].owners.is_empty());
}

#[test]
fn git_remote_parser_normalizes_github_origin() {
    let parsed =
        adapter::git::parse_github_remote_for_tests("git@github.com:example/review-firewall.git")
            .expect("parsed remote");
    assert_eq!(parsed.host, "github.com");
    assert_eq!(parsed.full_name, "example/review-firewall");
}

#[test]
fn git_status_parser_normalizes_windows_rename_paths() {
    let parsed = adapter::git::parse_status_paths_for_tests(
        "R  src\\new.rs\0src\\old.rs\0?? crates\\rf-core\\src\\lib.rs\0",
    );
    assert_eq!(parsed, vec!["src/new.rs", "crates/rf-core/src/lib.rs"]);
}

#[test]
fn git_status_parser_keeps_modified_file_prefixes_intact() {
    let parsed = adapter::git::parse_status_paths_for_tests(" M README.md\0");

    assert_eq!(parsed, vec!["README.md"]);
}

#[test]
fn git_status_parser_keeps_spaces_from_porcelain_z() {
    let parsed = adapter::git::parse_status_paths_for_tests("?? docs/has space.md\0");

    assert_eq!(parsed, vec!["docs/has space.md"]);
}

#[test]
fn git_changed_files_prefers_worktree_status_over_previous_commit() {
    let repo = temp_dir("changed-files-status");
    run_git(&repo, &["init", "--initial-branch=main"]);
    run_git(&repo, &["config", "user.email", "review-bot@example.com"]);
    run_git(&repo, &["config", "user.name", "Review Bot"]);
    fs::write(repo.join("stale.rs"), "initial\n").expect("write stale initial");
    run_git(&repo, &["add", "stale.rs"]);
    run_git(&repo, &["commit", "-m", "initial"]);
    fs::write(repo.join("stale.rs"), "second\n").expect("write stale second");
    run_git(&repo, &["add", "stale.rs"]);
    run_git(&repo, &["commit", "-m", "second"]);
    fs::write(repo.join("current.rs"), "worktree\n").expect("write current");

    let changed = adapter::git::changed_files(&repo, None);

    assert_eq!(changed.paths, vec![String::from("current.rs")]);
    assert!(changed.reason.is_none());
}

#[test]
fn git_changed_files_does_not_use_previous_commit_when_status_is_empty() {
    let repo = temp_dir("changed-files-clean");
    run_git(&repo, &["init", "--initial-branch=main"]);
    run_git(&repo, &["config", "user.email", "review-bot@example.com"]);
    run_git(&repo, &["config", "user.name", "Review Bot"]);
    fs::write(repo.join("first.rs"), "first\n").expect("write first");
    run_git(&repo, &["add", "first.rs"]);
    run_git(&repo, &["commit", "-m", "first"]);
    fs::write(repo.join("second.rs"), "second\n").expect("write second");
    run_git(&repo, &["add", "second.rs"]);
    run_git(&repo, &["commit", "-m", "second"]);

    let changed = adapter::git::changed_files(&repo, None);

    assert!(changed.paths.is_empty());
    assert_eq!(
        changed.reason.as_deref(),
        Some("No local changed files detected")
    );
}

#[test]
fn git_remote_parser_uses_any_configured_github_remote() {
    let parsed = adapter::git::parse_repository_identity_from_remotes_for_tests(
        "upstream\tgit@github.com:example/review-firewall.git (fetch)\nupstream\tgit@github.com:example/review-firewall.git (push)\n",
    )
    .expect("parsed remote");

    assert_eq!(parsed.host, "github.com");
    assert_eq!(parsed.full_name, "example/review-firewall");
}

#[test]
fn git_remote_parser_prefers_origin_fetch() {
    let parsed = adapter::git::parse_repository_identity_from_remotes_for_tests(
        "mirror\tgit@github.com:example/mirror.git (fetch)\nmirror\tgit@github.com:example/mirror.git (push)\norigin\tgit@github.com:example/review-firewall.git (fetch)\norigin\tgit@github.com:example/review-firewall.git (push)\n",
    )
    .expect("parsed remote");

    assert_eq!(parsed.host, "github.com");
    assert_eq!(parsed.full_name, "example/review-firewall");
}

#[test]
fn git_remote_parser_supports_enterprise_hosts() {
    let ssh = adapter::git::parse_github_remote_for_tests(
        "git@ghe.example.com:example/review-firewall.git",
    )
    .expect("parsed ssh remote");
    let https = adapter::git::parse_github_remote_for_tests(
        "https://ghe.example.com/example/review-firewall.git",
    )
    .expect("parsed https remote");

    assert_eq!(ssh.host, "ghe.example.com");
    assert_eq!(ssh.full_name, "example/review-firewall");
    assert_eq!(https.host, "ghe.example.com");
    assert_eq!(https.full_name, "example/review-firewall");
}

#[test]
fn scan_normalization_from_gh_fixture_preserves_expected_shape() {
    let pr_value: Value =
        serde_json::from_str(include_str!("fixtures/gh_pr_view.json")).expect("pr fixture");
    let comments_value: Value =
        serde_json::from_str(include_str!("fixtures/gh_review_comments.json"))
            .expect("comments fixture");

    let pr = command::scan::build_pull_request_summary_for_tests(&pr_value);
    let comments = comments_value
        .as_array()
        .expect("comment array")
        .iter()
        .filter_map(command::scan::review_comment_record_for_tests)
        .collect::<Vec<_>>();

    assert_eq!(pr.number, Some(142));
    assert_eq!(pr.title, "Refactor response handling");
    assert_eq!(pr.review_decisions[0], "CHANGES_REQUESTED");
    assert_eq!(comments.len(), 2);
    assert_eq!(comments[0].path.as_deref(), Some("src/api/response.rs"));
}

#[test]
fn scan_normalization_drops_empty_review_decisions() {
    let pr_value = serde_json::json!({
        "number": 143,
        "title": "Draft bootstrap",
        "reviewDecision": "",
        "reviews": [{ "state": "" }]
    });

    let pr = command::scan::build_pull_request_summary_for_tests(&pr_value);

    assert!(pr.review_decisions.is_empty());
}

#[test]
fn scan_changed_files_supplements_pr_file_list_with_local_diff() {
    let merged = command::scan::merge_changed_files_for_tests(
        vec![String::from("src/a.rs"), String::from("src/b.rs")],
        &[String::from("src/b.rs"), String::from("src/c.rs")],
    );

    assert_eq!(
        merged,
        vec![
            String::from("src/a.rs"),
            String::from("src/b.rs"),
            String::from("src/c.rs")
        ]
    );
}

#[test]
fn scan_changed_files_reads_paged_api_filenames() {
    let values = vec![
        serde_json::json!({ "filename": "src\\api.rs" }),
        serde_json::json!({ "path": "docs/contract.md" }),
    ];

    let parsed = command::scan::api_changed_files_for_tests(&values);

    assert_eq!(parsed, vec!["src/api.rs", "docs/contract.md"]);
}

#[test]
fn gh_paged_array_preserves_items_when_later_page_fails() {
    let first_page = (0..100)
        .map(|index| serde_json::json!({ "id": index }))
        .collect::<Vec<_>>();

    let probe = adapter::gh::collect_paged_arrays_for_tests(vec![
        Ok(serde_json::Value::Array(first_page)),
        Err(String::from("rate limited")),
    ])
    .expect("partial page result");

    assert_eq!(probe.values.len(), 100);
    assert_eq!(probe.reason.as_deref(), Some("rate limited"));
}

#[test]
fn scan_review_replies_share_root_thread_id() {
    let mut comments = vec![
        comment("12", None, CommentSource::ReviewComment),
        comment("13", Some("12"), CommentSource::ReviewComment),
    ];

    command::scan::normalize_review_comment_thread_ids_for_tests(&mut comments);

    assert_eq!(comments[0].thread_id, "12");
    assert_eq!(comments[1].thread_id, "12");
}

#[test]
fn scan_issue_comments_share_pseudo_thread_id() {
    let mut comments = vec![
        comment("22", None, CommentSource::IssueComment),
        comment("21", None, CommentSource::IssueComment),
    ];
    comments[0].created_at = Some(String::from("2026-03-28T00:00:01Z"));
    comments[1].created_at = Some(String::from("2026-03-28T00:00:00Z"));

    command::scan::normalize_issue_comment_thread_ids_for_tests(&mut comments);

    assert_eq!(comments[0].thread_id, "issue:21");
    assert_eq!(comments[1].thread_id, "issue:21");
}

#[test]
fn report_action_count_includes_more_than_four_actions() {
    let count = command::report::count_author_actions_for_tests(
        "## Author action list\n1. First\n2. Second\n3. Third\n4. Fourth\n5. Fifth\n10. Tenth\n",
    );

    assert_eq!(count, 6);
}

fn comment(id: &str, reply_to: Option<&str>, source: CommentSource) -> CommentRecord {
    CommentRecord {
        comment_id: id.to_owned(),
        thread_id: id.to_owned(),
        author: String::from("reviewer"),
        body: String::from("Body"),
        path: Some(String::from("src/api.rs")),
        source,
        reply_to_comment_id: reply_to.map(ToOwned::to_owned),
        created_at: Some(format!("2026-03-28T00:00:{id}Z")),
        line: None,
        original_line: None,
    }
}
