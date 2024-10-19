use crate::{global_config, model::ActivationLayer};
use std::process::Stdio;
use tokio::{
    io::{self, AsyncBufReadExt as _, BufReader},
    process::{ChildStderr, Command},
};
use tokio_stream::{wrappers::LinesStream, StreamExt};

pub async fn run_cmd(
    source_block_name: &'static str,
    cmd: &str,
    monitor: u8,
    layer: u16,
) -> io::Result<String> {
    let mut spawned = Command::new("bash")
        .args(["-c", cmd])
        .envs(global_config::get().as_env_vars(monitor, layer))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // although we wait for the child to terminate, we can't guarantee that
        // this future is not canceled, so we should enable kill on drop in
        // case the future is canceled to make this cancel safer.
        .kill_on_drop(true)
        .spawn()?;
    child_debug_loop(
        spawned.stderr.take().unwrap(),
        source_block_name,
        monitor,
        ActivationLayer::L(layer),
    );
    let output = spawned.wait_with_output().await?;
    if !output.status.success() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            String::from_utf8_lossy(&output.stdout),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub fn child_debug_loop(
    stderr: ChildStderr,
    name: &'static str,
    monitor: u8,
    layer: ActivationLayer,
) {
    if log::log_enabled!(log::Level::Debug) {
        tokio::spawn(async move {
            let mut stderr = LinesStream::new(BufReader::new(stderr).lines());
            while let Some(line) = stderr.next().await.transpose()? {
                log::debug!("[stderr of {name} @ mon:{monitor} in layer:{layer:?}] {line}")
            }

            Ok::<_, io::Error>(())
        });
    }
}
