use std::env;
use std::path::PathBuf;

use crate::command::{self, CommandKind, CommandOutcome};

pub fn run() -> i32 {
    match parse_args(env::args().skip(1).collect()) {
        ParseResult::Help => {
            print_help();
            0
        }
        ParseResult::Error(reason) => {
            eprintln!("STATUS: ERROR");
            eprintln!("REASON: {reason}");
            2
        }
        ParseResult::Command(command) => match dispatch(
            command,
            env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        ) {
            Ok(outcome) => {
                print_outcome(&outcome);
                0
            }
            Err(reason) => {
                eprintln!("STATUS: ERROR");
                eprintln!("REASON: {reason}");
                1
            }
        },
    }
}

enum ParseResult {
    Help,
    Error(String),
    Command(CommandKind),
}

fn parse_args(args: Vec<String>) -> ParseResult {
    let Some(command) = args.first() else {
        return ParseResult::Help;
    };

    match command.as_str() {
        "help" | "--help" => ParseResult::Help,
        "scan" => parse_scan_args(&args[1..]),
        "gate" if args.len() == 1 => ParseResult::Command(CommandKind::Gate),
        "draft-reply" if args.len() == 1 => ParseResult::Command(CommandKind::DraftReply),
        "escalate" if args.len() == 1 => ParseResult::Command(CommandKind::Escalate),
        "report" if args.len() == 1 => ParseResult::Command(CommandKind::Report),
        "gate" | "draft-reply" | "escalate" | "report" => {
            ParseResult::Error(format!("Unknown option for command `{command}`"))
        }
        other => ParseResult::Error(format!("Unknown command `{other}`")),
    }
}

fn parse_scan_args(args: &[String]) -> ParseResult {
    if args.is_empty() {
        return ParseResult::Command(CommandKind::Scan { pr: None });
    }

    if args.len() == 2 && args[0] == "--pr" {
        return match args[1].parse::<u64>() {
            Ok(value) if value > 0 => ParseResult::Command(CommandKind::Scan { pr: Some(value) }),
            _ => ParseResult::Error(String::from("--pr requires a positive integer")),
        };
    }

    ParseResult::Error(String::from("Usage: review-firewall scan [--pr <number>]"))
}

fn dispatch(command: CommandKind, cwd: PathBuf) -> Result<CommandOutcome, String> {
    match command {
        CommandKind::Scan { pr } => command::scan::run(&cwd, pr),
        CommandKind::Gate => command::gate::run(&cwd),
        CommandKind::DraftReply => command::draft_reply::run(&cwd),
        CommandKind::Escalate => command::escalate::run(&cwd),
        CommandKind::Report => command::report::run(&cwd),
    }
}

fn print_help() {
    println!("review-firewall v0.1");
    println!("Review the review. Protect the author.");
    println!();
    println!("Usage:");
    println!("  review-firewall scan [--pr <number>]   Gather PR and repo facts into scan.json");
    println!(
        "  review-firewall gate                   Classify comments and extract residual blockers"
    );
    println!("  review-firewall draft-reply            Generate a short author reply draft");
    println!("  review-firewall escalate               Move long design debates toward ADR/RFC");
    println!("  review-firewall report                 Build the final engineer/PM/author summary");
}

fn print_outcome(outcome: &CommandOutcome) {
    println!("STATUS: {}", outcome.status.terminal_label());
    for line in &outcome.lines {
        println!("{line}");
    }
    if let Some(reason) = outcome.reason.as_deref()
        && !reason.is_empty()
    {
        println!("REASON: {reason}");
    }
    if let Some(next) = outcome.next.as_deref()
        && !next.is_empty()
    {
        println!("NEXT: {next}");
    }
}
