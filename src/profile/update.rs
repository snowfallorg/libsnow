use crate::{profile::list::list, PackageAttr, PackageUpdate};
use anyhow::Result;
use tokio::process::Command;

pub async fn updatable() -> Result<Vec<PackageUpdate>> {
    let installed = list()?;
    let mut updatable = vec![];

    for pkg in installed {
        match &pkg.attr {
            PackageAttr::NixPkgs { attr } => {
                let output = Command::new("nix")
                    .arg("eval")
                    .arg(&format!("nixpkgs#{}.version", attr))
                    .arg("--raw")
                    .output()
                    .await?;
                let output = String::from_utf8(output.stdout)?;
                let version = output.trim();

                if version.is_empty() {
                    continue;
                } else if pkg.version.is_none() || version != pkg.version.as_ref().unwrap() {
                    updatable.push(PackageUpdate {
                        attr: pkg.attr.clone(),
                        new_version: version.to_string(),
                        old_version: pkg.version.unwrap_or_default(),
                    });
                }
            }
            PackageAttr::External { url, attr } => {
                let output = Command::new("nix")
                    .arg("eval")
                    .arg(&format!("{}#{}.version", url, attr))
                    .arg("--raw")
                    .output()
                    .await?;
                let output = String::from_utf8(output.stdout)?;
                let version = output.trim();

                if version.is_empty() {
                    continue;
                } else if pkg.version.is_none() || version != pkg.version.as_ref().unwrap() {
                    updatable.push(PackageUpdate {
                        attr: pkg.attr.clone(),
                        new_version: version.to_string(),
                        old_version: pkg.version.unwrap_or_default(),
                    });
                }
            }
        }
    }
    Ok(updatable)
}
