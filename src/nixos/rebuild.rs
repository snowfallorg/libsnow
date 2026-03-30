use super::AuthMethod;
use crate::{HELPER_EXEC, config::configfile::get_config, dbus};
use anyhow::{Result, anyhow};
use log::debug;

pub async fn rebuild(auth_method: AuthMethod<'_>) -> Result<()> {
    match auth_method {
        AuthMethod::Dbus => rebuild_dbus().await,
        _ => {
            let mut child = rebuild_spawn(auth_method)?;
            let status = child.wait().await?;
            debug!("{}", status);
            if !status.success() {
                return Err(anyhow!("Failed to rebuild"));
            }
            Ok(())
        }
    }
}

async fn rebuild_dbus() -> Result<()> {
    dbus::rebuild("switch").await
}

pub fn rebuild_spawn(auth_method: AuthMethod<'_>) -> Result<tokio::process::Child> {
    let config = get_config()?;
    let child = tokio::process::Command::new(match auth_method {
        AuthMethod::Dbus => unreachable!("D-Bus path handled in rebuild()"),
        AuthMethod::Sudo => "sudo",
        AuthMethod::Custom(cmd) => cmd,
    })
    .arg(HELPER_EXEC)
    .arg("rebuild")
    .args(if let Some(generations) = config.get_generation_count() {
        vec!["--generations".to_string(), generations.to_string()]
    } else {
        vec![]
    })
    .arg("--")
    .arg("switch")
    .args(if let Ok(flakedir) = config.get_flake_dir() {
        vec!["--flake".to_string(), flakedir]
    } else {
        vec![]
    })
    .spawn()?;

    Ok(child)
}
