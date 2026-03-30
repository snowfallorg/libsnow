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

#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.snowflakeos.LibSnow.Error")]
enum HelperError {
    #[zbus(error)]
    ZBus(zbus::Error),
    Busy(String),
    NotConfigured(String),
    InvalidAction(String),
    NotAuthorized(String),
    OperationFailed(String),
    NoOperation(String),
}

#[derive(serde::Deserialize, Default, Debug, Clone)]
struct LibSnowConfig {
    system_config_file: Option<String>,
    home_config_file: Option<String>,
    flake: Option<String>,
    host: Option<String>,
    generations: Option<u32>,
}

impl LibSnowConfig {
    fn load_system() -> Result<Self, HelperError> {
        let data = std::fs::read_to_string(SYSCONFIG)
            .map_err(|e| HelperError::NotConfigured(format!("Failed to read {SYSCONFIG}: {e}")))?;
        serde_json::from_str(&data)
            .map_err(|e| HelperError::NotConfigured(format!("Failed to parse {SYSCONFIG}: {e}")))
    }

    fn load_merged() -> Result<Self, HelperError> {
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
            (None, None) => Err(HelperError::NotConfigured(
                "No config found (/etc/libsnow/config.json or ~/.config/libsnow/config.json)"
                    .into(),
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

    fn system_config_path(&self) -> Result<String, HelperError> {
        self.system_config_file
            .clone()
            .ok_or_else(|| HelperError::NotConfigured("No system_config_file configured".into()))
    }

    fn home_config_path(&self) -> Result<String, HelperError> {
        self.home_config_file
            .clone()
            .ok_or_else(|| HelperError::NotConfigured("No home_config_file configured".into()))
    }

    fn flake_dir_or_err(&self) -> Result<String, HelperError> {
        self.flake_dir()
            .ok_or_else(|| HelperError::NotConfigured("No flake configured".into()))
    }
}

fn validate_action(action: &str) -> Result<(), HelperError> {
    if ALLOWED_ACTIONS.contains(&action) {
        Ok(())
    } else {
        Err(HelperError::InvalidAction(format!(
            "Action '{action}' is not allowed. Allowed: {ALLOWED_ACTIONS:?}"
        )))
    }
}

struct HelperState {
    active: bool,
    last_activity: Instant,
    active_sender: Option<String>,
}

/// PID of the currently running child process (0 = none).
pub static CHILD_PID: AtomicU32 = AtomicU32::new(0);

/// Shared inner logic used by both SystemHelper and UserHelper
struct HelperInner {
    state: Arc<Mutex<HelperState>>,
}

impl HelperInner {
    fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(HelperState {
                active: false,
                last_activity: Instant::now(),
                active_sender: None,
            })),
        }
    }

    fn begin_op(&self, sender: Option<&str>) -> Result<(), HelperError> {
        let mut state = self.state.lock().unwrap();
        if state.active {
            return Err(HelperError::Busy(
                "Another operation is already in progress".into(),
            ));
        }
        state.active = true;
        state.active_sender = sender.map(String::from);
        Ok(())
    }

    fn end_op(&self) {
        let mut state = self.state.lock().unwrap();
        state.active = false;
        state.active_sender = None;
        state.last_activity = Instant::now();
    }

    fn is_active_sender(&self, sender: &str) -> bool {
        let state = self.state.lock().unwrap();
        state.active_sender.as_deref() == Some(sender)
    }

    async fn run_op<F>(&self, sender: Option<&str>, f: F) -> Result<(), HelperError>
    where
        F: FnOnce() -> anyhow::Result<()> + Send + 'static,
    {
        self.begin_op(sender)?;
        let (tx, rx) = async_channel::bounded(1);
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
            let _ = tx.send_blocking(result);
        });
        let result = rx
            .recv()
            .await
            .map_err(|e| HelperError::OperationFailed(format!("Channel error: {e}")))?;
        self.end_op();
        match result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(HelperError::OperationFailed(e.to_string())),
            Err(_) => Err(HelperError::OperationFailed("Task panicked".into())),
        }
    }
}

fn cancel_child() -> Result<(), HelperError> {
    let pid = CHILD_PID.load(Ordering::SeqCst);
    if pid == 0 {
        return Err(HelperError::NoOperation("No operation in progress".into()));
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

fn sender_from_header(hdr: &zbus::message::Header<'_>) -> Result<String, HelperError> {
    hdr.sender()
        .map(|s| s.to_string())
        .ok_or_else(|| HelperError::OperationFailed("No sender in message header".into()))
}

async fn check_polkit_auth(
    connection: &Connection,
    hdr: &zbus::message::Header<'_>,
    action_id: &str,
) -> Result<(), HelperError> {
    use zbus_polkit::policykit1::{AuthorityProxy, CheckAuthorizationFlags, Subject};

    let subject = Subject::new_for_message_header(hdr)
        .map_err(|e| HelperError::OperationFailed(format!("Polkit subject error: {e}")))?;
    let proxy = AuthorityProxy::new(connection)
        .await
        .map_err(|e| HelperError::OperationFailed(format!("Polkit proxy error: {e}")))?;
    let result = proxy
        .check_authorization(
            &subject,
            action_id,
            &std::collections::HashMap::new(),
            CheckAuthorizationFlags::AllowUserInteraction.into(),
            "",
        )
        .await
        .map_err(|e| HelperError::OperationFailed(format!("Polkit check failed: {e}")))?;

    if !result.is_authorized {
        return Err(HelperError::NotAuthorized(
            "Not authorized by polkit".into(),
        ));
    }
    Ok(())
}

// System bus helper (runs as root, requires polkit authorization)
pub struct SystemHelper {
    inner: HelperInner,
}

#[interface(name = "org.snowflakeos.LibSnow.Helper1")]
impl SystemHelper {
    async fn config(
        &self,
        #[zbus(header)] hdr: zbus::message::Header<'_>,
        #[zbus(connection)] connection: &Connection,
        content: String,
        action: String,
    ) -> Result<(), HelperError> {
        check_polkit_auth(connection, &hdr, "org.snowflakeos.libsnow.config").await?;
        validate_action(&action)?;
        let sender = sender_from_header(&hdr)?;
        let cfg = LibSnowConfig::load_system()?;
        let output = cfg.system_config_path()?;
        let arguments = cfg.build_system_args(&action);
        let gens = cfg.generations;
        self.inner
            .run_op(Some(&sender), move || {
                operations::write_file(&output, arguments, gens, Some(content))
            })
            .await
    }

    async fn update(
        &self,
        #[zbus(header)] hdr: zbus::message::Header<'_>,
        #[zbus(connection)] connection: &Connection,
        action: String,
    ) -> Result<(), HelperError> {
        check_polkit_auth(connection, &hdr, "org.snowflakeos.libsnow.update").await?;
        validate_action(&action)?;
        let sender = sender_from_header(&hdr)?;
        let cfg = LibSnowConfig::load_system()?;
        let flake_dir = cfg.flake_dir_or_err()?;
        let arguments = cfg.build_system_args(&action);
        let gens = cfg.generations;
        self.inner
            .run_op(Some(&sender), move || {
                operations::update(&flake_dir, arguments, gens)
            })
            .await
    }

    async fn rebuild(
        &self,
        #[zbus(header)] hdr: zbus::message::Header<'_>,
        #[zbus(connection)] connection: &Connection,
        action: String,
    ) -> Result<(), HelperError> {
        check_polkit_auth(connection, &hdr, "org.snowflakeos.libsnow.rebuild").await?;
        validate_action(&action)?;
        let sender = sender_from_header(&hdr)?;
        let cfg = LibSnowConfig::load_system()?;
        let arguments = cfg.build_system_args(&action);
        let gens = cfg.generations;
        self.inner
            .run_op(Some(&sender), move || operations::rebuild(arguments, gens))
            .await
    }

    async fn cancel(
        &self,
        #[zbus(header)] hdr: zbus::message::Header<'_>,
        #[zbus(connection)] connection: &Connection,
    ) -> Result<(), HelperError> {
        let sender = sender_from_header(&hdr)?;
        if !self.inner.is_active_sender(&sender) {
            check_polkit_auth(connection, &hdr, "org.snowflakeos.libsnow.cancel").await?;
        }
        cancel_child()
    }
}

// Session bus helper (runs as user, no polkit)
pub struct UserHelper {
    inner: HelperInner,
}

#[interface(name = "org.snowflakeos.LibSnow.UserHelper1")]
impl UserHelper {
    async fn config_home(&self, content: String, action: String) -> Result<(), HelperError> {
        validate_action(&action)?;
        let cfg = LibSnowConfig::load_merged()?;
        let output = cfg.home_config_path()?;
        let arguments = cfg.build_home_args(&action);
        let gens = cfg.generations;
        self.inner
            .run_op(None, move || {
                operations::write_file_home(&output, arguments, gens, Some(content))
            })
            .await
    }

    async fn update_home(&self, action: String) -> Result<(), HelperError> {
        validate_action(&action)?;
        let cfg = LibSnowConfig::load_merged()?;
        let flake_dir = cfg.flake_dir_or_err()?;
        let arguments = cfg.build_home_args(&action);
        let gens = cfg.generations;
        self.inner
            .run_op(None, move || {
                operations::update_home(&flake_dir, arguments, gens)
            })
            .await
    }

    async fn rebuild_home(&self, action: String) -> Result<(), HelperError> {
        validate_action(&action)?;
        let cfg = LibSnowConfig::load_merged()?;
        let arguments = cfg.build_home_args(&action);
        let gens = cfg.generations;
        self.inner
            .run_op(None, move || operations::rebuild_home(arguments, gens))
            .await
    }

    async fn cancel(&self) -> Result<(), HelperError> {
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
    let inner = HelperInner::new();
    let idle_state = inner.state.clone();
    let helper = SystemHelper { inner };

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
    let inner = HelperInner::new();
    let idle_state = inner.state.clone();
    let helper = UserHelper { inner };

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
