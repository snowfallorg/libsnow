use crate::{
    nixos::list::list_systempackages, utils, PackageUpdate
};
use anyhow::Result;

pub async fn updatable(db: &rusqlite::Connection) -> Result<Vec<PackageUpdate>> {
    utils::misc::updatable(list_systempackages(db)?).await
}
