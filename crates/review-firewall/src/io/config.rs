use std::fs;
use std::path::Path;

use rf_core::domain::{GateConfigSnapshot, Status};

#[derive(Debug, Clone)]
pub struct ReviewSettings {
    pub max_pr_thread_roundtrips: usize,
}

#[derive(Debug, Clone)]
pub struct BlockerSettings {
    pub require_failure_mode: bool,
    pub require_concern: bool,
    pub require_evidence: bool,
    pub require_alternative: bool,
}

#[derive(Debug, Clone)]
pub struct OwnershipSettings {
    pub use_codeowners: bool,
}

#[derive(Debug, Clone)]
pub struct ReplySettings {
    pub max_lines: usize,
}

#[derive(Debug, Clone)]
pub struct ReviewFirewallConfig {
    pub found: bool,
    pub status: Status,
    pub reason: Option<String>,
    pub review: ReviewSettings,
    pub blocker: BlockerSettings,
    pub ownership: OwnershipSettings,
    pub reply: ReplySettings,
    pub unknown_keys: Vec<String>,
}

impl Default for ReviewFirewallConfig {
    fn default() -> Self {
        Self {
            found: false,
            status: Status::Ok,
            reason: None,
            review: ReviewSettings {
                max_pr_thread_roundtrips: 2,
            },
            blocker: BlockerSettings {
                require_failure_mode: true,
                require_concern: true,
                require_evidence: true,
                require_alternative: false,
            },
            ownership: OwnershipSettings {
                use_codeowners: true,
            },
            reply: ReplySettings { max_lines: 3 },
            unknown_keys: Vec::new(),
        }
    }
}

impl ReviewFirewallConfig {
    pub fn gate_snapshot(&self) -> GateConfigSnapshot {
        GateConfigSnapshot {
            require_failure_mode: self.blocker.require_failure_mode,
            require_concern: self.blocker.require_concern,
            require_evidence: self.blocker.require_evidence,
            require_alternative: self.blocker.require_alternative,
            max_pr_thread_roundtrips: self.review.max_pr_thread_roundtrips,
            use_codeowners: self.ownership.use_codeowners,
        }
    }
}

pub fn load(repo_root: &Path) -> ReviewFirewallConfig {
    let path = repo_root.join("review-firewall.toml");
    let mut config = ReviewFirewallConfig {
        ..ReviewFirewallConfig::default()
    };

    let content = match fs::read_to_string(&path) {
        Ok(content) => {
            config.found = true;
            content
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return config,
        Err(error) => {
            config.status = Status::Partial;
            config.reason = Some(error.to_string());
            return config;
        }
    };

    let mut section = String::new();
    for raw_line in content.lines() {
        let line = strip_comment(raw_line);
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = line.trim_matches(['[', ']']).to_owned();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            mark_partial(&mut config, format!("Invalid config line: {raw_line}"));
            continue;
        };
        assign_value(&mut config, section.as_str(), key.trim(), value.trim());
    }

    if !config.unknown_keys.is_empty() {
        let ignored_keys = config.unknown_keys.join(", ");
        mark_partial(
            &mut config,
            format!("Ignored unsupported config keys: {ignored_keys}"),
        );
    }

    config
}

fn assign_value(config: &mut ReviewFirewallConfig, section: &str, key: &str, raw_value: &str) {
    let scoped = if section.is_empty() {
        key.to_owned()
    } else {
        format!("{section}.{key}")
    };

    match scoped.as_str() {
        "version" => {
            if parse_usize(raw_value) != Some(1) {
                mark_partial(config, format!("Unsupported config version: {raw_value}"));
            }
        }
        "review.max_pr_thread_roundtrips" => {
            if let Some(value) = parse_usize(raw_value) {
                config.review.max_pr_thread_roundtrips = value;
            }
        }
        "blocker.require_failure_mode" => {
            if let Some(value) = parse_bool(raw_value) {
                config.blocker.require_failure_mode = value;
            }
        }
        "blocker.require_concern" => {
            if let Some(value) = parse_bool(raw_value) {
                config.blocker.require_concern = value;
            }
        }
        "blocker.require_evidence" => {
            if let Some(value) = parse_bool(raw_value) {
                config.blocker.require_evidence = value;
            }
        }
        "blocker.require_alternative" => {
            if let Some(value) = parse_bool(raw_value) {
                config.blocker.require_alternative = value;
            }
        }
        "ownership.use_codeowners" => {
            if let Some(value) = parse_bool(raw_value) {
                config.ownership.use_codeowners = value;
            }
        }
        "reply.max_lines" => {
            if let Some(value) = parse_usize(raw_value) {
                config.reply.max_lines = value.max(1);
            }
        }
        _ => config.unknown_keys.push(scoped),
    }
}

fn strip_comment(line: &str) -> String {
    let mut output = String::new();
    let mut in_string = false;
    for character in line.chars() {
        if character == '"' {
            in_string = !in_string;
        }
        if character == '#' && !in_string {
            break;
        }
        output.push(character);
    }
    output.trim().to_owned()
}

fn parse_bool(raw_value: &str) -> Option<bool> {
    match raw_value.trim() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn parse_usize(raw_value: &str) -> Option<usize> {
    raw_value.trim().parse().ok()
}

fn mark_partial(config: &mut ReviewFirewallConfig, reason: String) {
    config.status = config.status.merge(Status::Partial);
    if config.reason.is_none() {
        config.reason = Some(reason);
    }
}
