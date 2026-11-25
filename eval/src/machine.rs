use std::process::Command;

use crate::summary::MachineProfile;

pub(crate) fn machine_profile() -> MachineProfile {
    MachineProfile {
        os: std::env::consts::OS.to_owned(),
        arch: std::env::consts::ARCH.to_owned(),
        logical_cpus: std::thread::available_parallelism()
            .map(|count| count.get() as u64)
            .unwrap_or(1),
        cpu_model: cpu_model(),
        memory_bytes: memory_bytes(),
    }
}

#[cfg(target_os = "macos")]
fn sysctl(name: &str) -> Option<String> {
    let output = Command::new("sysctl").args(["-n", name]).output().ok()?;
    let text = String::from_utf8(output.stdout).ok()?;
    Some(text.trim().to_owned()).filter(|value| !value.is_empty())
}

#[cfg(target_os = "macos")]
fn cpu_model() -> Option<String> {
    sysctl("machdep.cpu.brand_string")
}

#[cfg(target_os = "macos")]
fn memory_bytes() -> Option<u64> {
    sysctl("hw.memsize")?.parse().ok()
}

#[cfg(target_os = "linux")]
fn proc_field(file: &str, field: &str) -> Option<String> {
    let text = std::fs::read_to_string(file).ok()?;
    text.lines()
        .find_map(|line| line.strip_prefix(field))
        .and_then(|rest| rest.split_once(':'))
        .map(|(_, value)| value.trim().to_owned())
}

#[cfg(target_os = "linux")]
fn cpu_model() -> Option<String> {
    proc_field("/proc/cpuinfo", "model name")
}

#[cfg(target_os = "linux")]
fn memory_bytes() -> Option<u64> {
    let kilobytes = proc_field("/proc/meminfo", "MemTotal")?;
    kilobytes
        .trim_end_matches("kB")
        .trim()
        .parse::<u64>()
        .ok()
        .map(|value| value * 1024)
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn cpu_model() -> Option<String> {
    None
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn memory_bytes() -> Option<u64> {
    None
}

pub(crate) fn codeint_version(codeint: &std::path::Path) -> Result<String, crate::EvaluationError> {
    crate::command::stdout_text(Command::new(codeint).arg("--version"))
}
