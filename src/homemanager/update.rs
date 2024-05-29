use crate::{
    homemanager::list::list, utils, PackageUpdate
};
use anyhow::Result;

pub async fn updatable(db: &rusqlite::Connection) -> Result<Vec<PackageUpdate>> {
    utils::misc::updatable(list(db)?).await
}
