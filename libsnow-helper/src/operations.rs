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

pub fn write_file(
    path: &str,
    args: Vec<String>,
    generations: Option<u32>,
    content: Option<String>,
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

    // If the user sends a SIGINT, restore the original configuration file
    {
        let mut signals = Signals::new([SIGINT]).unwrap();
        let p = path.to_string();
        let b = backup.clone();
        let handle = move || {
            let mut file = File::create(&p).unwrap();
            write!(file, "{}", &b).unwrap();
        };
        thread::spawn(move || {
            for sig in signals.forever() {
                if sig == SIGINT {
                    handle();
                    std::process::exit(1);
                }
            }
        });
    }

    let mut file = File::create(path)?;
    write!(file, "{}", &buf)?;

    if let Err(e) = rebuild(args, generations) {
        let mut file = File::create(path)?;
        write!(file, "{}", &backup)?;
        Err(e)
    } else {
        Ok(())
    }
}

pub fn update(path: &str, args: Vec<String>, generations: Option<u32>) -> Result<()> {
    let lock_path = format!("{}/flake.lock", path.trim_end_matches('/'));
    let lock_backup = fs::read_to_string(&lock_path).ok();

    // If the user sends a SIGINT, restore the original flake.lock
    if let Some(ref backup) = lock_backup {
        let mut signals = Signals::new([SIGINT]).unwrap();
        let p = lock_path.clone();
        let b = backup.clone();
        let handle = move || {
            let mut file = File::create(&p).unwrap();
            write!(file, "{}", &b).unwrap();
        };
        thread::spawn(move || {
            for sig in signals.forever() {
                if sig == SIGINT {
                    handle();
                    std::process::exit(1);
                }
            }
        });
    }

    let x = spawn_tracked(
        Command::new("nix")
            .arg("flake")
            .arg("update")
            .arg("--flake")
            .arg(path),
    )?;
    if !x.success() {
        // Restore flake.lock on update failure
        if let Some(backup) = lock_backup {
            let mut file = File::create(&lock_path)?;
            write!(file, "{}", &backup)?;
        }
        return Err(anyhow!(
            "nix flake update failed with exit code {}",
            exit_code_str(&x)
        ));
    }

    if let Err(e) = rebuild(args, generations) {
        // Restore flake.lock on rebuild failure
        if let Some(backup) = lock_backup {
            let mut file = File::create(&lock_path)?;
            write!(file, "{}", &backup)?;
        }
        Err(e)
    } else {
        Ok(())
    }
}

pub fn rebuild(args: Vec<String>, generations: Option<u32>) -> Result<()> {
    let x = spawn_tracked(Command::new("nixos-rebuild").args(&args))?;
    if !x.success() {
        return Err(anyhow!(
            "nixos-rebuild failed with exit code {}",
            exit_code_str(&x)
        ));
    }
    if let Some(g) = generations {
        if g > 0 {
            let x = spawn_tracked(
                Command::new("nix-env")
                    .arg("--delete-generations")
                    .arg("-p")
                    .arg("/nix/var/nix/profiles/system")
                    .arg(format!("+{}", g)),
            )?;
            if !x.success() {
                return Err(anyhow!(
                    "nix-env --delete-generations failed with exit code {}",
                    exit_code_str(&x)
                ));
            }
        }
    }
    Ok(())
}

pub fn write_file_home(
    path: &str,
    args: Vec<String>,
    generations: Option<u32>,
    content: Option<String>,
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

    // If the user sends a SIGINT, restore the original configuration file
    {
        let mut signals = Signals::new([SIGINT]).unwrap();
        let p = path.to_string();
        let b = backup.clone();
        let handle = move || {
            let mut file = File::create(&p).unwrap();
            write!(file, "{}", &b).unwrap();
        };
        thread::spawn(move || {
            for sig in signals.forever() {
                if sig == SIGINT {
                    handle();
                    std::process::exit(1);
                }
            }
        });
    }

    let mut file = File::create(path)?;
    write!(file, "{}", &buf)?;

    if let Err(e) = rebuild_home(args, generations) {
        let mut file = File::create(path)?;
        write!(file, "{}", &backup)?;
        Err(e)
    } else {
        Ok(())
    }
}

pub fn update_home(path: &str, args: Vec<String>, generations: Option<u32>) -> Result<()> {
    let lock_path = format!("{}/flake.lock", path.trim_end_matches('/'));
    let lock_backup = fs::read_to_string(&lock_path).ok();

    // If the user sends a SIGINT, restore the original flake.lock
    if let Some(ref backup) = lock_backup {
        let mut signals = Signals::new([SIGINT]).unwrap();
        let p = lock_path.clone();
        let b = backup.clone();
        let handle = move || {
            let mut file = File::create(&p).unwrap();
            write!(file, "{}", &b).unwrap();
        };
        thread::spawn(move || {
            for sig in signals.forever() {
                if sig == SIGINT {
                    handle();
                    std::process::exit(1);
                }
            }
        });
    }

    let x = spawn_tracked(
        Command::new("nix")
            .arg("flake")
            .arg("update")
            .arg("--flake")
            .arg(path),
    )?;
    if !x.success() {
        if let Some(backup) = lock_backup {
            let mut file = File::create(&lock_path)?;
            write!(file, "{}", &backup)?;
        }
        return Err(anyhow!(
            "nix flake update failed with exit code {}",
            exit_code_str(&x)
        ));
    }

    if let Err(e) = rebuild_home(args, generations) {
        if let Some(backup) = lock_backup {
            let mut file = File::create(&lock_path)?;
            write!(file, "{}", &backup)?;
        }
        Err(e)
    } else {
        Ok(())
    }
}

pub fn rebuild_home(args: Vec<String>, generations: Option<u32>) -> Result<()> {
    let x = spawn_tracked(Command::new("home-manager").args(&args))?;
    if !x.success() {
        return Err(anyhow!(
            "home-manager failed with exit code {}",
            exit_code_str(&x)
        ));
    }
    if let Some(g) = generations {
        if g > 0 {
            let x = spawn_tracked(
                Command::new("nix-env")
                    .arg("--delete-generations")
                    .arg("-p")
                    .arg(format!(
                        "{}/.local/state/nix/profiles/home-manager",
                        std::env::var("HOME")?
                    ))
                    .arg(format!("+{}", g)),
            )?;
            if !x.success() {
                return Err(anyhow!(
                    "nix-env --delete-generations failed with exit code {}",
                    exit_code_str(&x)
                ));
            }
        }
    }
    Ok(())
}
