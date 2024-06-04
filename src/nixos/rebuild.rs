use super::AuthMethod;
use crate::{config::configfile::get_config, HELPER_EXEC};
use anyhow::Result;
use log::debug;

pub async fn rebuild(auth_method: AuthMethod<'_>) -> Result<()> {
    let config = get_config()?;
    let output = tokio::process::Command::new(match auth_method {
        AuthMethod::Pkexec => "pkexec",
        AuthMethod::Sudo => "sudo",
        AuthMethod::Custom(cmd) => cmd,
    })
    .arg(&*HELPER_EXEC)
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
    .status()
    .await?;
    debug!("{}", output);
    Ok(())
}
