
use std::fmt::Result;

use libsnow::{homemanager, metadata::{database::{database_connection, fetch_database}, revision::{get_profile_revision, get_revision}}, nixos::{AuthMethod, list::{list_references, list_systempackages}}, profile::{install::install, list::list, update::updatable}, utils::misc::get_pname_from_storepath, NIXARCH};

#[tokio::main]
async fn main() {

    pretty_env_logger::init();

    // println!("{:#?}", fetch_database("b06025f1533a1e07b6db3e75151caa155d1c7eb3").await);

    let db = database_connection().await.unwrap();

    // let pkgs = list_references().await.unwrap().get_attributes();
    // let pkgs = homemanager::list::list(&db).unwrap();

    // println!("{:#?}", libsnow::nixos::remove::remove(&["vulkan-validation-layers"], &db, AuthMethod::Pkexec).await);
    // println!("{:#?}", libsnow::nixos::update::update(AuthMethod::Sudo).await);
    // println!("{:#?}", libsnow::nixenv::remove::remove(&["pandoc","lsd"]).await);
    println!("{:#?}", libsnow::homemanager::install::install(&["cambalache", "neofetch"], &db).await);

    // println!("{:#?}", &*libsnow::HELPER_EXEC);

    // println!("{:#?}", get_revision().await);
    // println!("{:#?}", list_references().await.unwrap());
    // println!("{:#?}", install("hello").await);
}
