
use std::fmt::Result;

use libsnow::{config::configfile::get_config_file, homemanager, metadata::{database::{database_connection, fetch_database}, revision::{get_profile_revision, get_revision}}, nixos::list::{list_references, list_systempackages}, profile::{install::install, list::list, update::updatable}, utils::misc::get_pname_from_storepath, NIXARCH};

#[tokio::main]
async fn main() {

    pretty_env_logger::init();

    // println!("{:#?}", fetch_database("b06025f1533a1e07b6db3e75151caa155d1c7eb3").await);

    let db = database_connection().await.unwrap();

    // let pkgs = list_references().await.unwrap().get_attributes();
    // let pkgs = homemanager::list::list(&db).unwrap();

    println!("{:#?}", libsnow::nixenv::update::updatable().await);

    // println!("{:#?}", get_revision().await);
    // println!("{:#?}", list_references().await.unwrap());
    // println!("{:#?}", install("hello").await);
}
