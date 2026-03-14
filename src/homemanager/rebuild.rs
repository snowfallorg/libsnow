use crate::{HELPER_EXEC, config::configfile::get_config, nixos::AuthMethod};
use anyhow::Result;
use log::debug;

pub async fn rebuild(auth_method: AuthMethod<'_>) -> Result<()> {
    let config = get_config()?;
    if config.system_for_home_manager {
        return crate::nixos::rebuild::rebuild(auth_method).await;
    };
    let output = tokio::process::Command::new(HELPER_EXEC)
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
        .status()
        .await?;
    debug!("{}", output);
    Ok(())
}
