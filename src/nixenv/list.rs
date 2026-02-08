use std::collections::HashMap;

use anyhow::Result;
use serde::Deserialize;

use crate::{Package, PackageAttr, metadata::Metadata};

#[derive(Debug, Deserialize, Clone)]
struct EnvPackage {
    pname: String,
}

pub async fn list(md: &Metadata) -> Result<Vec<Package>> {
    let output = std::process::Command::new("nix-env")
        .arg("-q")
        .arg("--out-path")
        .arg("--installed")
        .arg("--json")
        .output()?;
    let stdout = String::from_utf8(output.stdout)?;
    let packages: HashMap<String, EnvPackage> = serde_json::from_str(&stdout)?;

    let mut pkgs = Vec::new();

    for (_name, pkg) in packages {
        let matches = md.get_by_pname(&pkg.pname)?;
        // Only include the package if there's exactly one match (unambiguous)
        if matches.len() == 1 {
            let info = &matches[0];
            pkgs.push(Package {
                attr: PackageAttr::NixPkgs {
                    attr: info.attribute.clone(),
                },
                version: if info.version.is_empty() {
                    None
                } else {
                    Some(info.version.clone())
                },
                pname: Some(pkg.pname),
                ..Default::default()
            });
        }
    }

    Ok(pkgs)
}
