use std::path::Path;
use std::process::Command;

pub mod gh;
pub mod git;

#[derive(Debug, Clone)]
pub struct ProcessOutput {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub reason: Option<String>,
}

pub fn run_process(cwd: &Path, program: &str, args: &[String]) -> ProcessOutput {
    match Command::new(program).args(args).current_dir(cwd).output() {
        Ok(output) => ProcessOutput {
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            reason: None,
        },
        Err(error) => ProcessOutput {
            success: false,
            stdout: String::new(),
            stderr: String::new(),
            reason: Some(error.to_string()),
        },
    }
}
