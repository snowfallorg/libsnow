use crate::{HELPER_EXEC, config::configfile::get_config, dbus, nixos::AuthMethod};
use anyhow::{Result, anyhow};
use tracing::debug;

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
    let config = get_config()?;
    if config.system_for_home_manager {
        dbus::rebuild("switch").await
    } else {
        dbus::rebuild_home("switch").await
    }
}

pub fn rebuild_spawn(auth_method: AuthMethod<'_>) -> Result<tokio::process::Child> {
    let config = get_config()?;
    if config.system_for_home_manager {
        return crate::nixos::rebuild::rebuild_spawn(auth_method);
    };
    let child = tokio::process::Command::new(HELPER_EXEC)
        .arg("rebuild-home")
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
