#![allow(dead_code)]

use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[path = "../src/adapter/mod.rs"]
mod adapter;
#[path = "../src/command/mod.rs"]
mod command;
#[path = "../src/io/mod.rs"]
mod io;

use serde_json::Value;

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
        "R  src\\old.rs -> src\\new.rs\n?? crates\\rf-core\\src\\lib.rs\n",
    );
    assert_eq!(parsed, vec!["src/new.rs", "crates/rf-core/src/lib.rs"]);
}

#[test]
fn git_status_parser_keeps_modified_file_prefixes_intact() {
    let parsed = adapter::git::parse_status_paths_for_tests(" M README.md\n");

    assert_eq!(parsed, vec!["README.md"]);
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
