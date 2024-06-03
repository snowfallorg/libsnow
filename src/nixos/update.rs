use super::AuthMethod;
use crate::{
    config::configfile::get_config, nixos::list::list_systempackages, utils, PackageUpdate,
    HELPER_EXEC,
};
use anyhow::Result;
use log::debug;

pub async fn updatable(db: &rusqlite::Connection) -> Result<Vec<PackageUpdate>> {
    utils::misc::updatable(list_systempackages(db)?).await
}

pub async fn update(auth_method: AuthMethod<'_>) -> Result<()> {
    let config = get_config()?;
    let output = tokio::process::Command::new(match auth_method {
        AuthMethod::Pkexec => "pkexec",
        AuthMethod::Sudo => "sudo",
        AuthMethod::Custom(cmd) => cmd,
    })
    .arg(&*HELPER_EXEC)
    .arg("update")
    .args(if let Some(generations) = config.get_generation_count() {
        vec!["--generations".to_string(), generations.to_string()]
    } else {
        vec![]
    })
    .args(if let Ok(flakedir) = config.get_flake_dir() {
        vec!["--flake".to_string(), flakedir]
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
    .status()
    .await?;
    debug!("{}", output);
    Ok(())
}
