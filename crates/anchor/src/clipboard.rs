use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::{bail, Context, Result};

pub fn copy_with_timeout(text: &str, timeout: Duration) -> Result<()> {
    write_clipboard(text)?;
    spawn_clear_process(timeout)?;
    Ok(())
}

fn spawn_clear_process(timeout: Duration) -> Result<()> {
    let sleep = format_sleep_duration(timeout);
    Command::new("sh")
        .arg("-c")
        .arg(
            "sleep \"$1\"; if command -v wl-copy >/dev/null 2>&1; then printf '' | wl-copy; elif command -v xclip >/dev/null 2>&1; then printf '' | xclip -selection clipboard; fi",
        )
        .arg("anchor-clipboard-clear")
        .arg(sleep)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("failed to start clipboard clear process")?;
    Ok(())
}

fn write_clipboard(text: &str) -> Result<()> {
    if run_clip_command("wl-copy", std::iter::empty::<&str>(), text).is_ok() {
        return Ok(());
    }

    if run_clip_command("xclip", ["-selection", "clipboard"], text).is_ok() {
        return Ok(());
    }

    bail!("clipboard integration failed")
}

fn run_clip_command<I, S>(program: &str, args: I, text: &str) -> Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("failed to start {program}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        stdin
            .write_all(text.as_bytes())
            .with_context(|| format!("failed to write clipboard contents to {program}"))?;
    }

    let status = child
        .wait()
        .with_context(|| format!("failed to finish {program}"))?;

    if status.success() {
        Ok(())
    } else {
        bail!("{program} failed")
    }
}

fn format_sleep_duration(timeout: Duration) -> String {
    let seconds = timeout.as_secs();
    let millis = timeout.subsec_millis();
    format!("{seconds}.{millis:03}")
}
