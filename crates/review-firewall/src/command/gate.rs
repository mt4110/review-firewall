use std::path::Path;

use rf_core::domain::{GateArtifact, ScanArtifact, Status};
use rf_core::gate_scan;

use crate::adapter::git;
use crate::command::CommandOutcome;
use crate::io::{artifacts, codeowners, config, run_store};

pub fn run(cwd: &Path) -> Result<CommandOutcome, String> {
    let repo_root = git::repo_root(cwd);
    let run = run_store::latest_or_create(&repo_root.path).map_err(io_error)?;
    let scan =
        artifacts::read_json::<ScanArtifact>(run.directory.join("scan.json")).map_err(io_error)?;

    let mut artifact = if let Some(scan) = scan {
        let policy = config::load(&repo_root.path);
        let codeowners_file = codeowners::load(&repo_root.path);
        let mut gate = gate_scan(&scan, &policy.gate_snapshot(), &codeowners_file.rules);
        if let Some(config_reason) = policy.reason {
            gate.status = gate.status.merge(policy.status);
            if gate.reason.is_none() {
                gate.reason = Some(config_reason.clone());
            }
            gate.warnings.push(config_reason);
        }
        gate
    } else {
        missing_gate_artifact()
    };

    artifacts::write_json(run.directory.join("gate.json"), &artifact).map_err(io_error)?;

    let mut lines = vec![
        format!("Comments analyzed: {}", artifact.comments_analyzed),
        format!("Residual blockers: {}", artifact.residual_blockers.len()),
        format!("Questions: {}", artifact.counts.questions),
        format!("Suggestions: {}", artifact.counts.suggestions),
        format!("Nits: {}", artifact.counts.nits),
        format!("Praise: {}", artifact.counts.praise),
    ];
    if let Some(top) = artifact.residual_blockers.first() {
        lines.push(format!(
            "Top blocker: [{}] {} ({})",
            concern_label(&top.concern),
            top.failure_mode,
            weight_label(&top.advisory_weight)
        ));
    }

    Ok(CommandOutcome {
        status: artifact.status,
        reason: artifact.reason.take(),
        lines,
        next: None,
    })
}

fn missing_gate_artifact() -> GateArtifact {
    GateArtifact {
        status: Status::Error,
        reason: Some(String::from(
            "scan.json not found; run review-firewall scan first",
        )),
        comments_analyzed: 0,
        residual_blockers: Vec::new(),
        counts: Default::default(),
        candidate_blockers: Vec::new(),
        downgraded_comments: Vec::new(),
        duplicates_collapsed: Vec::new(),
        warnings: Vec::new(),
        config_snapshot: Default::default(),
        classified_comments: Vec::new(),
        escalation_candidates: Vec::new(),
    }
}

fn concern_label(concern: &rf_core::domain::BlockerConcern) -> &'static str {
    match concern {
        rf_core::domain::BlockerConcern::Correctness => "correctness",
        rf_core::domain::BlockerConcern::Security => "security",
        rf_core::domain::BlockerConcern::Performance => "performance",
        rf_core::domain::BlockerConcern::Operability => "operability",
        rf_core::domain::BlockerConcern::Api => "api",
    }
}

fn weight_label(weight: &rf_core::domain::AdvisoryWeight) -> &'static str {
    match weight {
        rf_core::domain::AdvisoryWeight::High => "high",
        rf_core::domain::AdvisoryWeight::Medium => "medium",
        rf_core::domain::AdvisoryWeight::Low => "low",
    }
}

fn io_error(error: std::io::Error) -> String {
    error.to_string()
}
