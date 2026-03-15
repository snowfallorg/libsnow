use crate::{
    HELPER_EXEC, PackageUpdate, config::configfile::get_config, homemanager::list::list, metadata::Metadata, nixos::AuthMethod, utils
};
use anyhow::Result;
use log::debug;

pub async fn updatable(md: &Metadata) -> Result<Vec<PackageUpdate>> {
    utils::misc::updatable(list(md)?).await
}

pub async fn update(auth_method: AuthMethod<'_>) -> Result<()> {
    let config = get_config()?;
    if config.system_for_home_manager {
        return crate::nixos::update::update(auth_method).await;
    };
    let output = tokio::process::Command::new(HELPER_EXEC)
        .arg("update-home")
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
