use std::path::Path;

use rf_core::domain::{EscalationLabel, ScanArtifact, Status};
use rf_core::{build_escalation_markdown, evaluate_escalation_candidates};

use crate::adapter::git;
use crate::command::CommandOutcome;
use crate::io::config::ReviewFirewallConfig;
use crate::io::{artifacts, config, run_store};

pub fn run(cwd: &Path) -> Result<CommandOutcome, String> {
    let repo_root = git::repo_root(cwd);
    let run = run_store::latest_or_create(&repo_root.path).map_err(io_error)?;
    let policy = config::load(&repo_root.path);
    let scan =
        artifacts::read_json::<ScanArtifact>(run.directory.join("scan.json")).map_err(io_error)?;

    let (mut status, mut reason, pr_number, candidates) = if let Some(scan) = scan {
        let candidates = evaluate_escalation_candidates(
            &scan.review_threads,
            policy.review.max_pr_thread_roundtrips,
        );
        (scan.status, scan.reason.clone(), scan.pr.number, candidates)
    } else {
        (
            Status::Error,
            Some(String::from(
                "scan.json not found; run review-firewall scan first",
            )),
            None,
            Vec::new(),
        )
    };
    merge_config_status(&mut status, &mut reason, &policy);

    let markdown = build_escalation_markdown(status, reason.as_deref(), pr_number, &candidates);
    artifacts::write_text(run.directory.join("escalation.md"), &markdown).map_err(io_error)?;

    let move_to_adr = candidates
        .iter()
        .filter(|candidate| candidate.label == EscalationLabel::MoveToAdr)
        .count();
    let move_to_rfc = candidates
        .iter()
        .filter(|candidate| candidate.label == EscalationLabel::MoveToRfc)
        .count();
    let needs_human = candidates
        .iter()
        .filter(|candidate| candidate.label == EscalationLabel::NeedsHumanJudgment)
        .count();

    Ok(CommandOutcome {
        status,
        reason,
        lines: vec![
            format!("Escalation candidates: {}", candidates.len()),
            format!("move_to_adr: {move_to_adr}"),
            format!("move_to_rfc: {move_to_rfc}"),
            format!("needs_human_judgment: {needs_human}"),
        ],
        next: None,
    })
}

fn merge_config_status(
    status: &mut Status,
    reason: &mut Option<String>,
    policy: &ReviewFirewallConfig,
) {
    if policy.status == Status::Ok {
        return;
    }
    *status = status.merge(policy.status);
    if reason.is_none() {
        *reason = policy
            .reason
            .clone()
            .or_else(|| Some(String::from("Config loaded with partial status")));
    }
}

fn io_error(error: std::io::Error) -> String {
    error.to_string()
}
