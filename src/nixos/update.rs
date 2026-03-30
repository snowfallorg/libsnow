use super::AuthMethod;
use crate::{
    HELPER_EXEC, PackageUpdate, config::configfile::get_config, dbus, metadata::Metadata,
    nixos::list::list_systempackages, utils,
};
use anyhow::{Result, anyhow};
use log::debug;

pub async fn updatable(md: &Metadata) -> Result<Vec<PackageUpdate>> {
    utils::misc::updatable(list_systempackages(md)?).await
}

pub async fn update(auth_method: AuthMethod<'_>) -> Result<()> {
    match auth_method {
        AuthMethod::Dbus => update_dbus().await,
        _ => {
            let mut child = update_spawn(auth_method)?;
            let status = child.wait().await?;
            debug!("{}", status);
            if !status.success() {
                return Err(anyhow!("Failed to rebuild"));
            }
            Ok(())
        }
    }
}

async fn update_dbus() -> Result<()> {
    dbus::update("switch").await
}

pub fn update_spawn(auth_method: AuthMethod<'_>) -> Result<tokio::process::Child> {
    let config = get_config()?;
    let child = tokio::process::Command::new(match auth_method {
        AuthMethod::Dbus => unreachable!("D-Bus path handled in update()"),
        AuthMethod::Sudo => "sudo",
        AuthMethod::Custom(cmd) => cmd,
    })
    .arg(HELPER_EXEC)
    .arg("update")
    .args(if let Some(generations) = config.get_generation_count() {
        vec!["--generations".to_string(), generations.to_string()]
    } else {
        vec![]
    })
    .args(if let Ok(flakedir) = config.get_flake_dir() {
        vec!["--flake".to_string(), flakedir.clone()]
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
