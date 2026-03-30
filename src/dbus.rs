use anyhow::Result;
use zbus::{Connection, proxy};

#[proxy(
    interface = "org.snowflakeos.LibSnow.Helper1",
    default_service = "org.snowflakeos.LibSnow.Helper1",
    default_path = "/org/snowflakeos/LibSnow/Helper1"
)]
trait Helper1 {
    fn config(&self, content: &str, action: &str) -> zbus::Result<()>;
    fn update(&self, action: &str) -> zbus::Result<()>;
    fn rebuild(&self, action: &str) -> zbus::Result<()>;
    fn cancel(&self) -> zbus::Result<()>;
}

#[proxy(
    interface = "org.snowflakeos.LibSnow.UserHelper1",
    default_service = "org.snowflakeos.LibSnow.UserHelper1",
    default_path = "/org/snowflakeos/LibSnow/UserHelper1"
)]
trait UserHelper1 {
    fn config_home(&self, content: &str, action: &str) -> zbus::Result<()>;
    fn update_home(&self, action: &str) -> zbus::Result<()>;
    fn rebuild_home(&self, action: &str) -> zbus::Result<()>;
    fn cancel(&self) -> zbus::Result<()>;
}

pub async fn config(content: &str, action: &str) -> Result<()> {
    let conn = Connection::system().await?;
    Ok(Helper1Proxy::new(&conn)
        .await?
        .config(content, action)
        .await?)
}

pub async fn update(action: &str) -> Result<()> {
    let conn = Connection::system().await?;
    Ok(Helper1Proxy::new(&conn).await?.update(action).await?)
}

pub async fn rebuild(action: &str) -> Result<()> {
    let conn = Connection::system().await?;
    Ok(Helper1Proxy::new(&conn).await?.rebuild(action).await?)
}

pub async fn cancel() -> Result<()> {
    let conn = Connection::system().await?;
    Ok(Helper1Proxy::new(&conn).await?.cancel().await?)
}

pub async fn config_home(content: &str, action: &str) -> Result<()> {
    let conn = Connection::session().await?;
    Ok(UserHelper1Proxy::new(&conn)
        .await?
        .config_home(content, action)
        .await?)
}

pub async fn update_home(action: &str) -> Result<()> {
    let conn = Connection::session().await?;
    Ok(UserHelper1Proxy::new(&conn)
        .await?
        .update_home(action)
        .await?)
}

pub async fn rebuild_home(action: &str) -> Result<()> {
    let conn = Connection::session().await?;
    Ok(UserHelper1Proxy::new(&conn)
        .await?
        .rebuild_home(action)
        .await?)
}

pub async fn cancel_home() -> Result<()> {
    let conn = Connection::session().await?;
    Ok(UserHelper1Proxy::new(&conn).await?.cancel().await?)
}
