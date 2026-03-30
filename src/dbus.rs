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

async fn system_proxy() -> Result<Helper1Proxy<'static>> {
    let conn = Connection::system().await?;
    let proxy = Helper1Proxy::new(&conn).await?;
    Ok(proxy)
}

pub async fn config(content: &str, action: &str) -> Result<()> {
    let proxy = system_proxy().await?;
    proxy.config(content, action).await?;
    Ok(())
}

pub async fn update(action: &str) -> Result<()> {
    let proxy = system_proxy().await?;
    proxy.update(action).await?;
    Ok(())
}

pub async fn rebuild(action: &str) -> Result<()> {
    let proxy = system_proxy().await?;
    proxy.rebuild(action).await?;
    Ok(())
}

pub async fn cancel() -> Result<()> {
    let proxy = system_proxy().await?;
    proxy.cancel().await?;
    Ok(())
}

async fn session_proxy() -> Result<UserHelper1Proxy<'static>> {
    let conn = Connection::session().await?;
    let proxy = UserHelper1Proxy::new(&conn).await?;
    Ok(proxy)
}

pub async fn config_home(content: &str, action: &str) -> Result<()> {
    let proxy = session_proxy().await?;
    proxy.config_home(content, action).await?;
    Ok(())
}

pub async fn update_home(action: &str) -> Result<()> {
    let proxy = session_proxy().await?;
    proxy.update_home(action).await?;
    Ok(())
}

pub async fn rebuild_home(action: &str) -> Result<()> {
    let proxy = session_proxy().await?;
    proxy.rebuild_home(action).await?;
    Ok(())
}

pub async fn cancel_home() -> Result<()> {
    let proxy = session_proxy().await?;
    proxy.cancel().await?;
    Ok(())
}
