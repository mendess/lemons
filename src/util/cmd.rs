use crate::global_config;
use std::process::Stdio;
use tokio::{io, process::Command};

pub async fn run_cmd(cmd: &str, monitor: u8, layer: u16) -> io::Result<String> {
    let spawned = Command::new("bash")
        .args(["-c", cmd])
        .envs(global_config::get().as_env_vars(monitor, layer))
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;
    let output = spawned.wait_with_output().await?;
    if !output.status.success() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            String::from_utf8_lossy(&output.stdout),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
