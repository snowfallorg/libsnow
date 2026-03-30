use crate::operations;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use zbus::{Connection, connection, interface};

const SYSTEM_BUS_NAME: &str = "org.snowflakeos.LibSnow.Helper1";
const SESSION_BUS_NAME: &str = "org.snowflakeos.LibSnow.UserHelper1";
const IDLE_TIMEOUT_SECS: u64 = 60;
const SYSCONFIG: &str = "/etc/libsnow/config.json";

/// Allowed rebuild actions
const ALLOWED_ACTIONS: &[&str] = &[
    "switch",
    "boot",
    "test",
    "build",
    "dry-build",
    "dry-activate",
];

#[derive(serde::Deserialize, Default, Debug, Clone)]
struct LibSnowConfig {
    system_config_file: Option<String>,
    home_config_file: Option<String>,
    flake: Option<String>,
    host: Option<String>,
    generations: Option<u32>,
}

impl LibSnowConfig {
    fn load_system() -> Result<Self, zbus::fdo::Error> {
        let data = std::fs::read_to_string(SYSCONFIG).map_err(|e| {
            zbus::fdo::Error::Failed(format!("Failed to read {}: {}", SYSCONFIG, e))
        })?;
        serde_json::from_str(&data)
            .map_err(|e| zbus::fdo::Error::Failed(format!("Failed to parse {}: {}", SYSCONFIG, e)))
    }

    fn load_merged() -> Result<Self, zbus::fdo::Error> {
        let sys: Option<Self> = std::fs::read_to_string(SYSCONFIG)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok());

        let home_config = dirs::config_dir()
            .map(|d| d.join("libsnow/config.json"))
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str::<LibSnowConfig>(&s).ok());

        match (sys, home_config) {
            (Some(s), Some(h)) => Ok(s.merge(h)),
            (Some(s), None) => Ok(s),
            (None, Some(h)) => Ok(h),
            (None, None) => Err(zbus::fdo::Error::Failed(
                "No config file found (checked /etc/libsnow/config.json and ~/.config/libsnow/config.json)".into(),
            )),
        }
    }

    fn merge(self, other: Self) -> Self {
        Self {
            flake: other.flake.or(self.flake),
            host: other.host.or(self.host),
            generations: other.generations.or(self.generations),
            system_config_file: other.system_config_file.or(self.system_config_file),
            home_config_file: other.home_config_file.or(self.home_config_file),
        }
    }

    fn flake_dir(&self) -> Option<String> {
        let flake = self.flake.as_ref()?;
        let p = Path::new(flake);
        if p.is_dir() {
            Some(flake.clone())
        } else {
            p.parent().map(|d| d.to_string_lossy().into_owned())
        }
    }

    fn build_system_args(&self, action: &str) -> Vec<String> {
        let mut args = vec![action.to_string()];
        if let Some(ref flake) = self.flake {
            args.push("--flake".to_string());
            let dir = self.flake_dir().unwrap_or_else(|| flake.clone());
            let flake_ref = if let Some(ref host) = self.host {
                format!("{}#{}", dir, host)
            } else {
                dir
            };
            args.push(flake_ref);
        }
        args
    }

    fn build_home_args(&self, action: &str) -> Vec<String> {
        let mut args = vec![action.to_string()];
        if let Some(ref flake) = self.flake {
            args.push("--flake".to_string());
            args.push(self.flake_dir().unwrap_or_else(|| flake.clone()));
        }
        args
    }

    fn system_config_path(&self) -> Result<String, zbus::fdo::Error> {
        self.system_config_file
            .clone()
            .ok_or_else(|| zbus::fdo::Error::Failed("No system_config_file configured".into()))
    }

    fn home_config_path(&self) -> Result<String, zbus::fdo::Error> {
        self.home_config_file
            .clone()
            .ok_or_else(|| zbus::fdo::Error::Failed("No home_config_file configured".into()))
    }

    fn flake_dir_or_err(&self) -> Result<String, zbus::fdo::Error> {
        self.flake_dir()
            .ok_or_else(|| zbus::fdo::Error::Failed("No flake configured".into()))
    }
}

fn validate_action(action: &str) -> Result<(), zbus::fdo::Error> {
    if ALLOWED_ACTIONS.contains(&action) {
        Ok(())
    } else {
        Err(zbus::fdo::Error::AccessDenied(format!(
            "Action '{}' is not allowed. Allowed: {:?}",
            action, ALLOWED_ACTIONS
        )))
    }
}

struct HelperState {
    active: bool,
    last_activity: Instant,
}

/// PID of the currently running child process (0 = none).
pub static CHILD_PID: AtomicU32 = AtomicU32::new(0);

fn cancel_child() -> Result<(), zbus::fdo::Error> {
    let pid = CHILD_PID.load(Ordering::SeqCst);
    if pid == 0 {
        return Err(zbus::fdo::Error::Failed("No operation in progress".into()));
    }
    unsafe {
        libc::kill(-(pid as i32), libc::SIGTERM);
    }
    std::thread::sleep(std::time::Duration::from_secs(2));
    let pid = CHILD_PID.load(Ordering::SeqCst);
    if pid != 0 {
        unsafe {
            libc::kill(-(pid as i32), libc::SIGKILL);
        }
    }
    Ok(())
}

// System bus helper (runs as root, requires polkit authorization)
pub struct SystemHelper {
    state: Arc<Mutex<HelperState>>,
}

impl SystemHelper {
    fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(HelperState {
                active: false,
                last_activity: Instant::now(),
            })),
        }
    }

    fn begin_op(&self) -> Result<(), zbus::fdo::Error> {
        let mut state = self.state.lock().unwrap();
        if state.active {
            return Err(zbus::fdo::Error::Failed(
                "Another operation is already in progress".into(),
            ));
        }
        state.active = true;
        Ok(())
    }

    fn end_op(&self) {
        let mut state = self.state.lock().unwrap();
        state.active = false;
        state.last_activity = Instant::now();
    }

    async fn run_op<F>(&self, f: F) -> Result<(), zbus::fdo::Error>
    where
        F: FnOnce() -> anyhow::Result<()> + Send + 'static,
    {
        self.begin_op()?;
        let (tx, rx) = async_channel::bounded(1);
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
            let _ = tx.send_blocking(result);
        });
        let result = rx
            .recv()
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("Channel error: {}", e)))?;
        self.end_op();
        match result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(zbus::fdo::Error::Failed(e.to_string())),
            Err(_) => Err(zbus::fdo::Error::Failed("Task panicked".into())),
        }
    }
}

async fn check_polkit_auth(
    connection: &Connection,
    hdr: &zbus::message::Header<'_>,
    action_id: &str,
) -> Result<(), zbus::fdo::Error> {
    use zbus_polkit::policykit1::{AuthorityProxy, CheckAuthorizationFlags, Subject};

    let subject = Subject::new_for_message_header(hdr)
        .map_err(|e| zbus::fdo::Error::Failed(format!("Polkit subject error: {}", e)))?;
    let proxy = AuthorityProxy::new(connection)
        .await
        .map_err(|e| zbus::fdo::Error::Failed(format!("Polkit proxy error: {}", e)))?;
    let result = proxy
        .check_authorization(
            &subject,
            action_id,
            &std::collections::HashMap::new(),
            CheckAuthorizationFlags::AllowUserInteraction.into(),
            "",
        )
        .await
        .map_err(|e| zbus::fdo::Error::Failed(format!("Polkit check failed: {}", e)))?;

    if !result.is_authorized {
        return Err(zbus::fdo::Error::AccessDenied(
            "Not authorized by polkit".into(),
        ));
    }
    Ok(())
}

#[interface(name = "org.snowflakeos.LibSnow.Helper1")]
impl SystemHelper {
    async fn config(
        &self,
        #[zbus(header)] hdr: zbus::message::Header<'_>,
        #[zbus(connection)] connection: &Connection,
        content: String,
        action: String,
    ) -> Result<(), zbus::fdo::Error> {
        check_polkit_auth(connection, &hdr, "org.snowflakeos.libsnow.config").await?;
        validate_action(&action)?;
        let cfg = LibSnowConfig::load_system()?;
        let output = cfg.system_config_path()?;
        let arguments = cfg.build_system_args(&action);
        let gens = cfg.generations;
        self.run_op(move || operations::write_file(&output, arguments, gens, Some(content)))
            .await
    }

    async fn update(
        &self,
        #[zbus(header)] hdr: zbus::message::Header<'_>,
        #[zbus(connection)] connection: &Connection,
        action: String,
    ) -> Result<(), zbus::fdo::Error> {
        check_polkit_auth(connection, &hdr, "org.snowflakeos.libsnow.update").await?;
        validate_action(&action)?;
        let cfg = LibSnowConfig::load_system()?;
        let flake_dir = cfg.flake_dir_or_err()?;
        let arguments = cfg.build_system_args(&action);
        let gens = cfg.generations;
        self.run_op(move || operations::update(&flake_dir, arguments, gens))
            .await
    }

    async fn rebuild(
        &self,
        #[zbus(header)] hdr: zbus::message::Header<'_>,
        #[zbus(connection)] connection: &Connection,
        action: String,
    ) -> Result<(), zbus::fdo::Error> {
        check_polkit_auth(connection, &hdr, "org.snowflakeos.libsnow.rebuild").await?;
        validate_action(&action)?;
        let cfg = LibSnowConfig::load_system()?;
        let arguments = cfg.build_system_args(&action);
        let gens = cfg.generations;
        self.run_op(move || operations::rebuild(arguments, gens))
            .await
    }

    async fn cancel(
        &self,
        #[zbus(header)] hdr: zbus::message::Header<'_>,
        #[zbus(connection)] connection: &Connection,
    ) -> Result<(), zbus::fdo::Error> {
        check_polkit_auth(connection, &hdr, "org.snowflakeos.libsnow.cancel").await?;
        cancel_child()
    }
}

// Session bus helper (runs as user, no polkit)
pub struct UserHelper {
    state: Arc<Mutex<HelperState>>,
}

impl UserHelper {
    fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(HelperState {
                active: false,
                last_activity: Instant::now(),
            })),
        }
    }

    fn begin_op(&self) -> Result<(), zbus::fdo::Error> {
        let mut state = self.state.lock().unwrap();
        if state.active {
            return Err(zbus::fdo::Error::Failed(
                "Another operation is already in progress".into(),
            ));
        }
        state.active = true;
        Ok(())
    }

    fn end_op(&self) {
        let mut state = self.state.lock().unwrap();
        state.active = false;
        state.last_activity = Instant::now();
    }

    async fn run_op<F>(&self, f: F) -> Result<(), zbus::fdo::Error>
    where
        F: FnOnce() -> anyhow::Result<()> + Send + 'static,
    {
        self.begin_op()?;
        let (tx, rx) = async_channel::bounded(1);
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
            let _ = tx.send_blocking(result);
        });
        let result = rx
            .recv()
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("Channel error: {}", e)))?;
        self.end_op();
        match result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(zbus::fdo::Error::Failed(e.to_string())),
            Err(_) => Err(zbus::fdo::Error::Failed("Task panicked".into())),
        }
    }
}

#[interface(name = "org.snowflakeos.LibSnow.UserHelper1")]
impl UserHelper {
    async fn config_home(&self, content: String, action: String) -> Result<(), zbus::fdo::Error> {
        validate_action(&action)?;
        let cfg = LibSnowConfig::load_merged()?;
        let output = cfg.home_config_path()?;
        let arguments = cfg.build_home_args(&action);
        let gens = cfg.generations;
        self.run_op(move || operations::write_file_home(&output, arguments, gens, Some(content)))
            .await
    }

    async fn update_home(&self, action: String) -> Result<(), zbus::fdo::Error> {
        validate_action(&action)?;
        let cfg = LibSnowConfig::load_merged()?;
        let flake_dir = cfg.flake_dir_or_err()?;
        let arguments = cfg.build_home_args(&action);
        let gens = cfg.generations;
        self.run_op(move || operations::update_home(&flake_dir, arguments, gens))
            .await
    }

    async fn rebuild_home(&self, action: String) -> Result<(), zbus::fdo::Error> {
        validate_action(&action)?;
        let cfg = LibSnowConfig::load_merged()?;
        let arguments = cfg.build_home_args(&action);
        let gens = cfg.generations;
        self.run_op(move || operations::rebuild_home(arguments, gens))
            .await
    }

    async fn cancel(&self) -> Result<(), zbus::fdo::Error> {
        cancel_child()
    }
}

async fn run_idle_loop(state: Arc<Mutex<HelperState>>) {
    let idle_timeout = std::time::Duration::from_secs(IDLE_TIMEOUT_SECS);
    loop {
        async_io::Timer::after(std::time::Duration::from_secs(10)).await;
        let state = state.lock().unwrap();
        if !state.active && state.last_activity.elapsed() >= idle_timeout {
            eprintln!("libsnow-helper: idle for {}s, exiting", IDLE_TIMEOUT_SECS);
            break;
        }
    }
}

pub async fn run_system_daemon() -> anyhow::Result<()> {
    let helper = SystemHelper::new();
    let idle_state = helper.state.clone();

    let _conn = connection::Builder::system()?
        .name(SYSTEM_BUS_NAME)?
        .serve_at("/org/snowflakeos/LibSnow/Helper1", helper)?
        .build()
        .await?;

    eprintln!(
        "libsnow-helper: system D-Bus daemon running on {}",
        SYSTEM_BUS_NAME
    );
    run_idle_loop(idle_state).await;
    Ok(())
}

pub async fn run_session_daemon() -> anyhow::Result<()> {
    let helper = UserHelper::new();
    let idle_state = helper.state.clone();

    let _conn = connection::Builder::session()?
        .name(SESSION_BUS_NAME)?
        .serve_at("/org/snowflakeos/LibSnow/UserHelper1", helper)?
        .build()
        .await?;

    eprintln!(
        "libsnow-helper: session D-Bus daemon running on {}",
        SESSION_BUS_NAME
    );
    run_idle_loop(idle_state).await;
    Ok(())
}
