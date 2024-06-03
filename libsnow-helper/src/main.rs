use anyhow::{anyhow, Result};
use clap::{self, FromArgMatches, Subcommand};
use signal_hook::{consts::SIGINT, iterator::Signals};
use std::{
    fs::{self, File},
    io::{self, Read, Write},
    process::Command,
    thread,
};

#[derive(Subcommand, Debug)]
enum SubCommands {
    Config {
        /// Write stdin to file in path output
        #[arg(short, long)]
        output: String,

        /// How many generations to keep
        #[arg(short, long)]
        generations: Option<u32>,

        /// Run `nixos-rebuild` with the given arguments
        arguments: Vec<String>,
    },
    Update {
        /// Path to flake file
        #[arg(short, long)]
        flake: String,

        /// How many generations to keep
        #[arg(short, long)]
        generations: Option<u32>,

        /// Run `nixos-rebuild` with the given arguments
        arguments: Vec<String>,
    },
    Rebuild {
        /// How many generations to keep
        #[arg(short, long)]
        generations: Option<u32>,

        /// Run `nixos-rebuild` with the given arguments
        arguments: Vec<String>,
    },
    ConfigHome {
        /// Write stdin to file in path output
        #[arg(short, long)]
        output: String,

        /// How many generations to keep
        #[arg(short, long)]
        generations: Option<u32>,

        /// Run `home-manager` with the given arguments
        arguments: Vec<String>,
    },
    UpdateHome {
        /// Path to flake file
        #[arg(short, long)]
        flake: String,

        /// How many generations to keep
        #[arg(short, long)]
        generations: Option<u32>,

        /// Run `nixos-rebuild` with the given arguments
        arguments: Vec<String>,
    },
    RebuildHome {
        /// How many generations to keep
        #[arg(short, long)]
        generations: Option<u32>,

        /// Run `home-manager` with the given arguments
        arguments: Vec<String>,
    },
}

fn main() {
    let cli = SubCommands::augment_subcommands(clap::Command::new("Helper binary for libsnow"));
    let matches = cli.get_matches();
    let derived_subcommands = SubCommands::from_arg_matches(&matches)
        .map_err(|err| err.exit())
        .unwrap();

    match derived_subcommands {
        SubCommands::Config {
            output,
            generations,
            arguments,
        } => {
            match write_file(&output, arguments, generations) {
                Ok(_) => (),
                Err(err) => {
                    eprintln!("{}", err);
                    std::process::exit(1);
                }
            };
        }
        SubCommands::Update {
            flake,
            generations,
            arguments,
        } => match update(&flake, arguments, generations) {
            Ok(_) => (),
            Err(err) => {
                eprintln!("{}", err);
                std::process::exit(1);
            }
        },
        SubCommands::Rebuild {
            generations,
            arguments,
        } => match rebuild(arguments, generations) {
            Ok(_) => (),
            Err(err) => {
                eprintln!("{}", err);
                std::process::exit(1);
            }
        },
        SubCommands::ConfigHome {
            output,
            generations,
            arguments,
        } => {
            match write_file_home(&output, arguments, generations) {
                Ok(_) => (),
                Err(err) => {
                    eprintln!("{}", err);
                    std::process::exit(1);
                }
            };
        },
        SubCommands::UpdateHome {
            flake,
            generations,
            arguments,
        } => match update_home(&flake, arguments, generations) {
            Ok(_) => (),
            Err(err) => {
                eprintln!("{}", err);
                std::process::exit(1);
            }
        },
        SubCommands::RebuildHome {
            generations,
            arguments,
        } => match rebuild_home(arguments, generations) {
            Ok(_) => (),
            Err(err) => {
                eprintln!("{}", err);
                std::process::exit(1);
            }
        },
    }
}

fn write_file(path: &str, args: Vec<String>, generations: Option<u32>) -> Result<()> {
    let backup = fs::read_to_string(path)?;

    let stdin = io::stdin();
    let mut buf = String::new();
    stdin.lock().read_to_string(&mut buf)?;

    // If the user sends a SIGINT, restore the original configuration file
    {
        let mut signals = Signals::new(&[SIGINT]).unwrap();
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

    if rebuild(args, generations).is_err() {
        let mut file = File::create(path)?;
        write!(file, "{}", &backup)?;
        Err(anyhow!("Failed to rebuild"))
    } else {
        Ok(())
    }
}

fn update(path: &str, args: Vec<String>, generations: Option<u32>) -> Result<()> {
    let mut cmd = Command::new("nix")
        .arg("flake")
        .arg("update")
        .arg("--flake")
        .arg(path)
        .spawn()?;
    let x = cmd.wait()?;
    if !x.success() {
        eprintln!(
            "nix flake update failed with exit code {}",
            x.code().unwrap()
        );
        std::process::exit(1);
    }
    rebuild(args, generations)
}

fn rebuild(args: Vec<String>, generations: Option<u32>) -> Result<()> {
    let mut cmd = Command::new("nixos-rebuild").args(args).spawn()?;
    let x = cmd.wait()?;
    if !x.success() {
        eprintln!("nixos-rebuild failed with exit code {}", x.code().unwrap());
        return Err(anyhow!("nixos-rebuild failed"));
    }
    if let Some(g) = generations {
        if g > 0 {
            let mut cmd = Command::new("nix-env")
                .arg("--delete-generations")
                .arg("-p")
                .arg("/nix/var/nix/profiles/system")
                .arg(&format!("+{}", g))
                .spawn()?;
            let x = cmd.wait()?;
            if !x.success() {
                eprintln!(
                    "nix-env --delete-generations failed with exit code {}",
                    x.code().unwrap()
                );
                return Err(anyhow!("nix-env failed"));
            }
        }
    }
    Ok(())
}

fn write_file_home(path: &str, args: Vec<String>, generations: Option<u32>) -> Result<()> {
    let backup = fs::read_to_string(path)?;

    let stdin = io::stdin();
    let mut buf = String::new();
    stdin.lock().read_to_string(&mut buf)?;

    // If the user sends a SIGINT, restore the original configuration file
    {
        let mut signals = Signals::new(&[SIGINT]).unwrap();
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

    if rebuild_home(args, generations).is_err() {
        let mut file = File::create(path)?;
        write!(file, "{}", &backup)?;
        Err(anyhow!("Failed to rebuild"))
    } else {
        Ok(())
    }
}

fn update_home(path: &str, args: Vec<String>, generations: Option<u32>) -> Result<()> {
    let mut cmd = Command::new("nix")
        .arg("flake")
        .arg("update")
        .arg("--flake")
        .arg(path)
        .spawn()?;
    let x = cmd.wait()?;
    if !x.success() {
        eprintln!(
            "nix flake update failed with exit code {}",
            x.code().unwrap()
        );
        std::process::exit(1);
    }
    rebuild_home(args, generations)
}

fn rebuild_home(args: Vec<String>, generations: Option<u32>) -> Result<()> {
    let mut cmd = Command::new("home-manager").args(args).spawn()?;
    let x = cmd.wait()?;
    if !x.success() {
        eprintln!("home-manager failed with exit code {}", x.code().unwrap());
        return Err(anyhow!("home-manager failed"));
    }
    if let Some(g) = generations {
        if g > 0 {
            let mut cmd = Command::new("nix-env")
                .arg("--delete-generations")
                .arg("-p")
                .arg(format!("{}/.local/state/nix/profiles/home-manager", std::env::var("HOME")?))
                .arg(&format!("+{}", g))
                .spawn()?;
            let x = cmd.wait()?;
            if !x.success() {
                eprintln!(
                    "nix-env --delete-generations failed with exit code {}",
                    x.code().unwrap()
                );
                return Err(anyhow!("nix-env failed"));
            }
        }
    }
    Ok(())
}