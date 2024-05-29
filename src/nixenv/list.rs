use std::collections::HashMap;

use anyhow::Result;
use serde::Deserialize;

use crate::{utils::{misc::get_pname_from_storepath, storedb::get_storebatch}, Package, PackageAttr};

#[derive(Debug, Deserialize, Clone)]
struct EnvPackage {
    outputs: EnvOutput,
}

#[derive(Debug, Deserialize, Clone)]
struct EnvOutput {
    out: String,
}

pub async fn list() -> Result<Vec<Package>> {
    let output = std::process::Command::new("nix-env")
        .arg("-q")
        .arg("--out-path")
        .arg("--installed")
        .arg("--json")
        .output()?;
    let stdout = String::from_utf8(output.stdout)?;
    let packages: HashMap<String, EnvPackage> = serde_json::from_str(&stdout)?;
    let paths = packages
        .values()
        .map(|x| x.outputs.out.split('/').last().unwrap_or(&x.outputs.out))
        .collect::<Vec<_>>();

    let storebatch = get_storebatch(paths.iter().map(AsRef::as_ref).collect()).await?;
    Ok(storebatch.packages.into_iter().map(|x| {
        Package {
            attr: PackageAttr::NixPkgs { attr: x.attribute.join(".") },
            version: x.version.clone(),
            pname: get_pname_from_storepath(x.store.as_str(), x.version).ok(),
        }
    }).collect())
}
