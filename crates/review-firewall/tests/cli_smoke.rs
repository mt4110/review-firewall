use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir(name: &str) -> PathBuf {
    let directory = env::temp_dir().join(format!(
        "review-firewall-{name}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("unix time")
            .as_nanos()
    ));
    fs::create_dir_all(&directory).expect("create temp directory");
    directory
}

fn init_repo(root: &Path) {
    run(Command::new("git")
        .arg("init")
        .arg("--initial-branch=main")
        .current_dir(root));
    run(Command::new("git")
        .args(["config", "user.email", "review-bot@example.com"])
        .current_dir(root));
    run(Command::new("git")
        .args(["config", "user.name", "Review Bot"])
        .current_dir(root));
    run(Command::new("git")
        .args([
            "remote",
            "add",
            "origin",
            "git@github.com:example/review-firewall.git",
        ])
        .current_dir(root));

    fs::create_dir_all(root.join("src")).expect("src dir");
    fs::create_dir_all(root.join(".github")).expect("codeowners dir");
    fs::write(
        root.join("src").join("api.rs"),
        "pub fn response_contract() {}\n",
    )
    .expect("write source");
    fs::write(
        root.join(".github").join("CODEOWNERS"),
        "/src/* @reviewer-a\n",
    )
    .expect("write codeowners");

    run(Command::new("git").args(["add", "."]).current_dir(root));
    run(Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(root));
}

fn install_gh_success_stub(root: &Path) -> PathBuf {
    let stub_dir = root.join("bin");
    fs::create_dir_all(&stub_dir).expect("stub dir");
    let gh_path = gh_stub_path(&stub_dir);
    fs::write(&gh_path, gh_success_stub_contents()).expect("write gh stub");
    make_executable(&gh_path);
    stub_dir
}

fn install_gh_error_stub(root: &Path) -> PathBuf {
    let stub_dir = root.join("bin-error");
    fs::create_dir_all(&stub_dir).expect("stub error dir");
    let gh_path = gh_stub_path(&stub_dir);
    fs::write(&gh_path, gh_error_stub_contents()).expect("write gh error stub");
    make_executable(&gh_path);
    stub_dir
}

#[cfg(unix)]
fn gh_stub_path(stub_dir: &Path) -> PathBuf {
    stub_dir.join("gh")
}

#[cfg(windows)]
fn gh_stub_path(stub_dir: &Path) -> PathBuf {
    stub_dir.join("gh.cmd")
}

#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path).expect("stub metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("chmod stub");
}

#[cfg(not(unix))]
fn make_executable(_: &Path) {}

#[cfg(unix)]
fn gh_success_stub_contents() -> &'static str {
    r#"#!/bin/sh
set -eu
if [ "$1" = "pr" ] && [ "$2" = "view" ]; then
cat <<'JSON'
{"number":42,"title":"Refactor response handling","body":"Body","author":{"login":"author"},"baseRefName":"main","headRefName":"feature/test","labels":[{"name":"api"}],"reviewDecision":"CHANGES_REQUESTED","files":[{"path":"src/api.rs"}],"reviews":[{"state":"CHANGES_REQUESTED"}],"url":"https://github.com/example/review-firewall/pull/42"}
JSON
exit 0
fi
if [ "$1" = "api" ]; then
case "$2" in
  "repos/example/review-firewall/pulls/42/files?per_page=100&page=1")
cat <<'JSON'
[{"filename":"src/api.rs"}]
JSON
exit 0
;;
  "repos/example/review-firewall/pulls/42/files?per_page=100&page=2")
echo '[]'
exit 0
;;
  "repos/example/review-firewall/pulls/42/comments?per_page=100&page=1")
cat <<'JSON'
[{"id":12,"body":"This can break the response contract in this PR because `partial` changes client handling.","path":"src/api.rs","user":{"login":"reviewer-a"},"pull_request_review_id":1,"created_at":"2026-03-28T00:00:00Z","line":10,"original_line":10},{"id":13,"body":"I agree the client contract changes here.","path":"src/api.rs","user":{"login":"author"},"pull_request_review_id":1,"created_at":"2026-03-28T00:00:01Z","in_reply_to_id":12,"line":10,"original_line":10}]
JSON
exit 0
;;
  "repos/example/review-firewall/pulls/42/comments?per_page=100&page=2")
echo '[]'
exit 0
;;
  "repos/example/review-firewall/issues/42/comments?per_page=100&page=1")
cat <<'JSON'
[{"id":21,"body":"This contract boundary affects consumers beyond this PR.","user":{"login":"reviewer-a"},"created_at":"2026-03-28T00:00:02Z"},{"id":22,"body":"I think we can keep the current schema in this PR.","user":{"login":"author"},"created_at":"2026-03-28T00:00:03Z"},{"id":23,"body":"The architecture discussion should move to an ADR before merge.","user":{"login":"reviewer-a"},"created_at":"2026-03-28T00:00:04Z"},{"id":24,"body":"If we keep debating the contract here, the PR will stall.","user":{"login":"author"},"created_at":"2026-03-28T00:00:05Z"}]
JSON
exit 0
;;
  "repos/example/review-firewall/issues/42/comments?per_page=100&page=2")
echo '[]'
exit 0
;;
esac
fi
echo "unexpected gh invocation" >&2
exit 1
"#
}

#[cfg(windows)]
fn gh_success_stub_contents() -> &'static str {
    r#"@echo off
setlocal
if "%1"=="pr" if "%2"=="view" (
  echo {"number":42,"title":"Refactor response handling","body":"Body","author":{"login":"author"},"baseRefName":"main","headRefName":"feature/test","labels":[{"name":"api"}],"reviewDecision":"CHANGES_REQUESTED","files":[{"path":"src/api.rs"}],"reviews":[{"state":"CHANGES_REQUESTED"}],"url":"https://github.com/example/review-firewall/pull/42"}
  exit /b 0
)
if "%1"=="api" (
  if "%~2"=="repos/example/review-firewall/pulls/42/files?per_page=100&page=1" (
    echo [{"filename":"src/api.rs"}]
    exit /b 0
  )
  if "%~2"=="repos/example/review-firewall/pulls/42/files?per_page=100&page=2" (
    echo []
    exit /b 0
  )
  if "%~2"=="repos/example/review-firewall/pulls/42/comments?per_page=100&page=1" (
    echo [{"id":12,"body":"This can break the response contract in this PR because `partial` changes client handling.","path":"src/api.rs","user":{"login":"reviewer-a"},"pull_request_review_id":1,"created_at":"2026-03-28T00:00:00Z","line":10,"original_line":10},{"id":13,"body":"I agree the client contract changes here.","path":"src/api.rs","user":{"login":"author"},"pull_request_review_id":1,"created_at":"2026-03-28T00:00:01Z","in_reply_to_id":12,"line":10,"original_line":10}]
    exit /b 0
  )
  if "%~2"=="repos/example/review-firewall/pulls/42/comments?per_page=100&page=2" (
    echo []
    exit /b 0
  )
  if "%~2"=="repos/example/review-firewall/issues/42/comments?per_page=100&page=1" (
    echo [{"id":21,"body":"This contract boundary affects consumers beyond this PR.","user":{"login":"reviewer-a"},"created_at":"2026-03-28T00:00:02Z"},{"id":22,"body":"I think we can keep the current schema in this PR.","user":{"login":"author"},"created_at":"2026-03-28T00:00:03Z"},{"id":23,"body":"The architecture discussion should move to an ADR before merge.","user":{"login":"reviewer-a"},"created_at":"2026-03-28T00:00:04Z"},{"id":24,"body":"If we keep debating the contract here, the PR will stall.","user":{"login":"author"},"created_at":"2026-03-28T00:00:05Z"}]
    exit /b 0
  )
  if "%~2"=="repos/example/review-firewall/issues/42/comments?per_page=100&page=2" (
    echo []
    exit /b 0
  )
)
>&2 echo unexpected gh invocation
exit /b 1
"#
}

#[cfg(unix)]
fn gh_error_stub_contents() -> &'static str {
    "#!/bin/sh\necho 'gh stub failure' >&2\nexit 1\n"
}

#[cfg(windows)]
fn gh_error_stub_contents() -> &'static str {
    "@echo off\r\n>&2 echo gh stub failure\r\nexit /b 1\r\n"
}

fn binary_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_review-firewall"))
}

fn run_with_path(repo: &Path, extra_path: &Path, args: &[&str]) -> std::process::Output {
    Command::new(binary_path())
        .args(args)
        .current_dir(repo)
        .env("PATH", joined_path(extra_path))
        .output()
        .expect("run review-firewall")
}

fn joined_path(extra_path: &Path) -> OsString {
    let mut paths = vec![extra_path.to_path_buf()];
    let existing = env::var_os("PATH").expect("PATH");
    paths.extend(env::split_paths(&existing));
    env::join_paths(paths).expect("join PATH")
}

fn latest_run_dir(repo: &Path) -> PathBuf {
    let latest = fs::read_to_string(
        repo.join(".review-firewall")
            .join("run")
            .join("latest.json"),
    )
    .expect("latest pointer");
    let latest: serde_json::Value = serde_json::from_str(&latest).expect("latest json");
    let timestamp = latest
        .get("timestamp")
        .and_then(serde_json::Value::as_str)
        .expect("timestamp");
    repo.join(".review-firewall").join("run").join(timestamp)
}

#[test]
fn smoke_flow_creates_all_artifacts() {
    let repo = temp_dir("smoke-ok");
    init_repo(&repo);
    let gh_stub = install_gh_success_stub(&repo);

    for args in [
        vec!["scan", "--pr", "42"],
        vec!["gate"],
        vec!["draft-reply"],
        vec!["escalate"],
        vec!["report"],
    ] {
        let output = run_with_path(&repo, &gh_stub, &args);
        assert!(
            output.status.success(),
            "command failed: {:?}\nstdout={}\nstderr={}",
            args,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let latest = fs::read_to_string(
        repo.join(".review-firewall")
            .join("run")
            .join("latest.json"),
    )
    .expect("latest pointer");
    let latest: serde_json::Value = serde_json::from_str(&latest).expect("latest json");
    let timestamp = latest
        .get("timestamp")
        .and_then(serde_json::Value::as_str)
        .expect("timestamp");
    let run_dir = repo.join(".review-firewall").join("run").join(timestamp);

    for file in [
        "scan.json",
        "gate.json",
        "draft_reply.json",
        "draft_reply.md",
        "escalation.md",
        "report.md",
    ] {
        assert!(run_dir.join(file).exists(), "missing artifact: {file}");
    }

    let scan = fs::read_to_string(run_dir.join("scan.json")).expect("scan");
    let gate = fs::read_to_string(run_dir.join("gate.json")).expect("gate");
    let escalation = fs::read_to_string(run_dir.join("escalation.md")).expect("escalation");
    assert!(scan.contains(r#""thread_id": "issue:21""#));
    assert!(scan.contains(r#""thread_id": "issue:22""#));
    assert!(!scan.contains("issue:conversation"));
    assert!(gate.contains(r#""status": "OK""#));
    assert!(gate.contains(r#""residual_blockers""#));
    assert!(escalation.contains("No ADR/RFC candidates were found."));
}

#[test]
fn report_preserves_error_status_when_upstream_artifact_is_missing() {
    let repo = temp_dir("report-preserve-error");
    init_repo(&repo);
    let gh_stub = install_gh_success_stub(&repo);

    for args in [vec!["scan", "--pr", "42"], vec!["gate"]] {
        let output = run_with_path(&repo, &gh_stub, &args);
        assert!(
            output.status.success(),
            "command failed: {:?}\nstdout={}\nstderr={}",
            args,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let run_dir = latest_run_dir(&repo);
    let gate_path = run_dir.join("gate.json");
    let mut gate: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&gate_path).expect("gate")).expect("gate json");
    gate["status"] = serde_json::Value::String(String::from("ERROR"));
    gate["reason"] = serde_json::Value::String(String::from("gate failed"));
    fs::write(
        &gate_path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&gate).expect("render gate")
        ),
    )
    .expect("write gate");

    let report = run_with_path(&repo, &gh_stub, &["report"]);

    assert!(report.status.success());
    let stdout = String::from_utf8_lossy(&report.stdout);
    assert!(stdout.contains("STATUS: ERROR"));
    assert!(stdout.contains("REASON: gate failed"));
    let report_md = fs::read_to_string(run_dir.join("report.md")).expect("report");
    assert!(report_md.contains("STATUS: ERROR"));
    assert!(report_md.contains("REASON: gate failed"));
}

#[test]
fn gate_writes_error_artifact_for_unreadable_scan_artifact() {
    let repo = temp_dir("gate-corrupt-scan");
    init_repo(&repo);
    let gh_stub = install_gh_success_stub(&repo);

    let scan = run_with_path(&repo, &gh_stub, &["scan", "--pr", "42"]);
    assert!(scan.status.success());
    let run_dir = latest_run_dir(&repo);
    fs::write(run_dir.join("scan.json"), "{ not json\n").expect("corrupt scan");

    let gate = run_with_path(&repo, &gh_stub, &["gate"]);

    assert!(gate.status.success());
    let stdout = String::from_utf8_lossy(&gate.stdout);
    assert!(stdout.contains("STATUS: ERROR"));
    assert!(stdout.contains("scan.json could not be read"));
    let gate_json = fs::read_to_string(run_dir.join("gate.json")).expect("gate");
    assert!(gate_json.contains(r#""status": "ERROR""#));
    assert!(gate_json.contains("scan.json could not be read"));
}

#[test]
fn draft_reply_writes_error_artifacts_for_unreadable_gate_artifact() {
    let repo = temp_dir("draft-corrupt-gate");
    init_repo(&repo);
    let gh_stub = install_gh_success_stub(&repo);

    for args in [vec!["scan", "--pr", "42"], vec!["gate"]] {
        let output = run_with_path(&repo, &gh_stub, &args);
        assert!(output.status.success(), "command failed: {:?}", args);
    }
    let run_dir = latest_run_dir(&repo);
    fs::write(run_dir.join("gate.json"), "{ not json\n").expect("corrupt gate");

    let draft = run_with_path(&repo, &gh_stub, &["draft-reply"]);

    assert!(draft.status.success());
    let stdout = String::from_utf8_lossy(&draft.stdout);
    assert!(stdout.contains("STATUS: ERROR"));
    assert!(stdout.contains("gate.json could not be read"));
    let draft_json = fs::read_to_string(run_dir.join("draft_reply.json")).expect("draft");
    let draft_md = fs::read_to_string(run_dir.join("draft_reply.md")).expect("draft md");
    assert!(draft_json.contains(r#""status": "ERROR""#));
    assert!(draft_json.contains("gate.json could not be read"));
    assert!(draft_md.contains("gate.json could not be read"));
    assert!(draft_md.contains("could not complete blocker analysis"));
    assert!(!draft_md.contains("does not think this blocks merge"));
}

#[test]
fn escalate_writes_error_artifact_for_unreadable_scan_artifact() {
    let repo = temp_dir("escalate-corrupt-scan");
    init_repo(&repo);
    let gh_stub = install_gh_success_stub(&repo);

    let scan = run_with_path(&repo, &gh_stub, &["scan", "--pr", "42"]);
    assert!(scan.status.success());
    let run_dir = latest_run_dir(&repo);
    fs::write(run_dir.join("scan.json"), "{ not json\n").expect("corrupt scan");

    let escalate = run_with_path(&repo, &gh_stub, &["escalate"]);

    assert!(escalate.status.success());
    let stdout = String::from_utf8_lossy(&escalate.stdout);
    assert!(stdout.contains("STATUS: ERROR"));
    assert!(stdout.contains("scan.json could not be read"));
    let escalation_md = fs::read_to_string(run_dir.join("escalation.md")).expect("escalation");
    assert!(escalation_md.contains("STATUS: ERROR"));
    assert!(escalation_md.contains("scan.json could not be read"));
}

#[test]
fn report_names_missing_escalation_artifact_reason() {
    let repo = temp_dir("report-missing-escalation");
    init_repo(&repo);
    let gh_stub = install_gh_success_stub(&repo);

    for args in [
        vec!["scan", "--pr", "42"],
        vec!["gate"],
        vec!["draft-reply"],
    ] {
        let output = run_with_path(&repo, &gh_stub, &args);
        assert!(
            output.status.success(),
            "command failed: {:?}\nstdout={}\nstderr={}",
            args,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let run_dir = latest_run_dir(&repo);
    let report = run_with_path(&repo, &gh_stub, &["report"]);

    assert!(report.status.success());
    let stdout = String::from_utf8_lossy(&report.stdout);
    assert!(stdout.contains("STATUS: PARTIAL"));
    assert!(stdout.contains("escalation.md not found; run review-firewall escalate first"));
    assert!(!stdout.contains("escalation.md is missing STATUS"));
    let report_md = fs::read_to_string(run_dir.join("report.md")).expect("report");
    assert!(report_md.contains("escalation.md not found; run review-firewall escalate first"));
    assert!(!report_md.contains("escalation.md is missing STATUS"));
}

#[test]
fn report_writes_error_artifact_for_unreadable_upstream_artifact() {
    let repo = temp_dir("report-corrupt-upstream");
    init_repo(&repo);
    let gh_stub = install_gh_success_stub(&repo);

    for args in [
        vec!["scan", "--pr", "42"],
        vec!["gate"],
        vec!["draft-reply"],
        vec!["escalate"],
    ] {
        let output = run_with_path(&repo, &gh_stub, &args);
        assert!(
            output.status.success(),
            "command failed: {:?}\nstdout={}\nstderr={}",
            args,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let run_dir = latest_run_dir(&repo);
    fs::write(run_dir.join("draft_reply.json"), "{ not json\n").expect("corrupt draft");

    let report = run_with_path(&repo, &gh_stub, &["report"]);

    assert!(report.status.success());
    let stdout = String::from_utf8_lossy(&report.stdout);
    assert!(stdout.contains("STATUS: ERROR"));
    assert!(stdout.contains("draft_reply.json could not be read"));
    let report_md = fs::read_to_string(run_dir.join("report.md")).expect("report");
    assert!(report_md.contains("STATUS: ERROR"));
    assert!(report_md.contains("draft_reply.json could not be read"));
}

#[test]
fn config_partial_status_reaches_reply_and_escalation_commands() {
    let repo = temp_dir("smoke-config-partial");
    init_repo(&repo);
    let gh_stub = install_gh_success_stub(&repo);

    for args in [vec!["scan", "--pr", "42"], vec!["gate"]] {
        let output = run_with_path(&repo, &gh_stub, &args);
        assert!(
            output.status.success(),
            "command failed: {:?}\nstdout={}\nstderr={}",
            args,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fs::write(
        repo.join("review-firewall.toml"),
        "[review]\nmax_pr_thread_roundtrips = nope\n[reply]\nmax_lines = nope\n",
    )
    .expect("write invalid config");

    let draft = run_with_path(&repo, &gh_stub, &["draft-reply"]);
    let escalate = run_with_path(&repo, &gh_stub, &["escalate"]);
    let report = run_with_path(&repo, &gh_stub, &["report"]);

    assert!(draft.status.success());
    assert!(escalate.status.success());
    assert!(report.status.success());
    let draft_stdout = String::from_utf8_lossy(&draft.stdout);
    let escalate_stdout = String::from_utf8_lossy(&escalate.stdout);
    let report_stdout = String::from_utf8_lossy(&report.stdout);
    assert!(draft_stdout.contains("STATUS: PARTIAL"));
    assert!(draft_stdout.contains("REASON: Invalid config value"));
    assert!(escalate_stdout.contains("STATUS: PARTIAL"));
    assert!(escalate_stdout.contains("REASON: Invalid config value"));
    assert!(report_stdout.contains("STATUS: PARTIAL"));
    assert!(report_stdout.contains("REASON: Invalid config value"));
}

#[test]
fn stopless_partial_path_still_writes_artifacts() {
    let repo = temp_dir("smoke-error");
    init_repo(&repo);
    run(Command::new("git")
        .args(["checkout", "-b", "feature/local-only"])
        .current_dir(&repo));
    fs::write(
        repo.join("src").join("local_only.rs"),
        "pub fn local_only_change() {}\n",
    )
    .expect("write local-only source");
    run(Command::new("git").args(["add", "."]).current_dir(&repo));
    run(Command::new("git")
        .args(["commit", "-m", "local-only change"])
        .current_dir(&repo));
    let gh_stub = install_gh_error_stub(&repo);

    for args in [
        vec!["scan"],
        vec!["gate"],
        vec!["draft-reply"],
        vec!["escalate"],
        vec!["report"],
    ] {
        let output = run_with_path(&repo, &gh_stub, &args);
        assert!(output.status.success(), "command failed: {:?}", args);
    }

    let latest = fs::read_to_string(
        repo.join(".review-firewall")
            .join("run")
            .join("latest.json"),
    )
    .expect("latest pointer");
    let latest: serde_json::Value = serde_json::from_str(&latest).expect("latest json");
    let timestamp = latest
        .get("timestamp")
        .and_then(serde_json::Value::as_str)
        .expect("timestamp");
    let run_dir = repo.join(".review-firewall").join("run").join(timestamp);

    let scan = fs::read_to_string(run_dir.join("scan.json")).expect("scan");
    let report = fs::read_to_string(run_dir.join("report.md")).expect("report");

    assert!(scan.contains(r#""status": "PARTIAL""#));
    assert!(scan.contains("gh stub failure"));
    assert!(scan.contains("src/local_only.rs"));
    assert!(report.contains("STATUS: PARTIAL"));
}

fn run(command: &mut Command) {
    let output = command.output().expect("run command");
    assert!(
        output.status.success(),
        "command failed\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
