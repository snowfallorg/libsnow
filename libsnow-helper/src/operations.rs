use anyhow::{Result, anyhow};
use signal_hook::{consts::SIGINT, iterator::Signals};
use std::{
    fs::{self, File},
    io::{self, Read, Write},
    os::unix::process::CommandExt,
    process::Command,
    sync::atomic::{AtomicBool, Ordering},
    thread,
};

use crate::dbus::CHILD_PID;

/// When true, children are placed in their own process group for D-Bus signal handling
static OWN_PROCESS_GROUP: AtomicBool = AtomicBool::new(false);

pub fn enable_process_groups() {
    OWN_PROCESS_GROUP.store(true, Ordering::SeqCst);
}

fn spawn_tracked(cmd: &mut Command) -> Result<std::process::ExitStatus> {
    if OWN_PROCESS_GROUP.load(Ordering::SeqCst) {
        cmd.process_group(0);
    }
    let mut child = cmd.spawn()?;
    CHILD_PID.store(child.id(), Ordering::SeqCst);
    let status = child.wait()?;
    CHILD_PID.store(0, Ordering::SeqCst);
    Ok(status)
}

fn exit_code_str(status: &std::process::ExitStatus) -> String {
    match status.code() {
        Some(code) => format!("{}", code),
        None => "killed by signal".to_string(),
    }
}

fn run_cmd(cmd: &mut Command, name: &str) -> Result<()> {
    let status = spawn_tracked(cmd)?;
    if !status.success() {
        return Err(anyhow!(
            "{} failed with exit code {}",
            name,
            exit_code_str(&status)
        ));
    }
    Ok(())
}

fn delete_generations(profile_path: &str, generations: Option<u32>) -> Result<()> {
    if let Some(g) = generations {
        if g > 0 {
            run_cmd(
                Command::new("nix-env")
                    .arg("--delete-generations")
                    .arg("-p")
                    .arg(profile_path)
                    .arg(format!("+{}", g)),
                "nix-env --delete-generations",
            )?;
        }
    }
    Ok(())
}

fn register_restore_on_sigint(path: &str, backup: &str) {
    let mut signals = Signals::new([SIGINT]).expect("failed to register SIGINT handler");
    let p = path.to_string();
    let b = backup.to_string();
    thread::spawn(move || {
        for sig in signals.forever() {
            if sig == SIGINT {
                let mut file = File::create(&p).expect("failed to restore file on SIGINT");
                write!(file, "{}", &b).expect("failed to write backup on SIGINT");
                std::process::exit(1);
            }
        }
    });
}

fn restore_backup(path: &str, backup: &Option<String>) -> Result<()> {
    if let Some(ref b) = backup {
        let mut file = File::create(path)?;
        write!(file, "{}", b)?;
    }
    Ok(())
}

fn write_file_impl(
    path: &str,
    args: Vec<String>,
    generations: Option<u32>,
    content: Option<String>,
    rebuild_fn: fn(Vec<String>, Option<u32>) -> Result<()>,
) -> Result<()> {
    let backup = fs::read_to_string(path)?;

    let buf = match content {
        Some(c) => c,
        None => {
            let stdin = io::stdin();
            let mut buf = String::new();
            stdin.lock().read_to_string(&mut buf)?;
            buf
        }
    };

    register_restore_on_sigint(path, &backup);

    let mut file = File::create(path)?;
    write!(file, "{}", &buf)?;

    if let Err(e) = rebuild_fn(args, generations) {
        let mut file = File::create(path)?;
        write!(file, "{}", &backup)?;
        Err(e)
    } else {
        Ok(())
    }
}

pub fn write_file(
    path: &str,
    args: Vec<String>,
    generations: Option<u32>,
    content: Option<String>,
) -> Result<()> {
    write_file_impl(path, args, generations, content, rebuild)
}

pub fn write_file_home(
    path: &str,
    args: Vec<String>,
    generations: Option<u32>,
    content: Option<String>,
) -> Result<()> {
    write_file_impl(path, args, generations, content, rebuild_home)
}

fn update_impl(
    path: &str,
    args: Vec<String>,
    generations: Option<u32>,
    rebuild_fn: fn(Vec<String>, Option<u32>) -> Result<()>,
) -> Result<()> {
    let lock_path = format!("{}/flake.lock", path.trim_end_matches('/'));
    let lock_backup = fs::read_to_string(&lock_path).ok();

    if let Some(ref backup) = lock_backup {
        register_restore_on_sigint(&lock_path, backup);
    }

    if let Err(e) = run_cmd(
        Command::new("nix")
            .arg("flake")
            .arg("update")
            .arg("--flake")
            .arg(path),
        "nix flake update",
    ) {
        restore_backup(&lock_path, &lock_backup)?;
        return Err(e);
    }

    if let Err(e) = rebuild_fn(args, generations) {
        restore_backup(&lock_path, &lock_backup)?;
        Err(e)
    } else {
        Ok(())
    }
}

pub fn update(path: &str, args: Vec<String>, generations: Option<u32>) -> Result<()> {
    update_impl(path, args, generations, rebuild)
}

pub fn update_home(path: &str, args: Vec<String>, generations: Option<u32>) -> Result<()> {
    update_impl(path, args, generations, rebuild_home)
}

pub fn rebuild(args: Vec<String>, generations: Option<u32>) -> Result<()> {
    run_cmd(Command::new("nixos-rebuild").args(&args), "nixos-rebuild")?;
    delete_generations("/nix/var/nix/profiles/system", generations)
}

pub fn rebuild_home(args: Vec<String>, generations: Option<u32>) -> Result<()> {
    run_cmd(Command::new("home-manager").args(&args), "home-manager")?;
    let profile_path = dirs::home_dir()
        .expect("could not determine home directory")
        .join(".local/state/nix/profiles/home-manager");
    delete_generations(&profile_path.to_string_lossy(), generations)
}
