mod dbus;
mod operations;

use clap::{self, FromArgMatches, Subcommand};

#[derive(Subcommand, Debug)]
enum SubCommands {
    /// Run as a D-Bus service
    Dbus {
        /// Run on the session bus instead of the system bus
        #[arg(long)]
        session: bool,
    },
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
        SubCommands::Dbus { session } => {
            operations::enable_process_groups();
            let result = if session {
                async_io::block_on(dbus::run_session_daemon())
            } else {
                async_io::block_on(dbus::run_system_daemon())
            };
            if let Err(err) = result {
                eprintln!("D-Bus daemon error: {}", err);
                std::process::exit(1);
            }
        }
        SubCommands::Config {
            output,
            generations,
            arguments,
        } => {
            match operations::write_file(&output, arguments, generations, None) {
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
        } => match operations::update(&flake, arguments, generations) {
            Ok(_) => (),
            Err(err) => {
                eprintln!("{}", err);
                std::process::exit(1);
            }
        },
        SubCommands::Rebuild {
            generations,
            arguments,
        } => match operations::rebuild(arguments, generations) {
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
            match operations::write_file_home(&output, arguments, generations, None) {
                Ok(_) => (),
                Err(err) => {
                    eprintln!("{}", err);
                    std::process::exit(1);
                }
            };
        }
        SubCommands::UpdateHome {
            flake,
            generations,
            arguments,
        } => match operations::update_home(&flake, arguments, generations) {
            Ok(_) => (),
            Err(err) => {
                eprintln!("{}", err);
                std::process::exit(1);
            }
        },
        SubCommands::RebuildHome {
            generations,
            arguments,
        } => match operations::rebuild_home(arguments, generations) {
            Ok(_) => (),
            Err(err) => {
                eprintln!("{}", err);
                std::process::exit(1);
            }
        },
    }
}
