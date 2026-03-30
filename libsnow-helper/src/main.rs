mod dbus;
mod operations;

use anyhow::Result;
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

    if let Err(err) = run(derived_subcommands) {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run(cmd: SubCommands) -> Result<()> {
    match cmd {
        SubCommands::Dbus { session } => {
            operations::enable_process_groups();
            if session {
                async_io::block_on(dbus::run_session_daemon())
            } else {
                async_io::block_on(dbus::run_system_daemon())
            }
        }
        SubCommands::Config {
            output,
            generations,
            arguments,
        } => operations::write_file(&output, arguments, generations, None),
        SubCommands::Update {
            flake,
            generations,
            arguments,
        } => operations::update(&flake, arguments, generations),
        SubCommands::Rebuild {
            generations,
            arguments,
        } => operations::rebuild(arguments, generations),
        SubCommands::ConfigHome {
            output,
            generations,
            arguments,
        } => operations::write_file_home(&output, arguments, generations, None),
        SubCommands::UpdateHome {
            flake,
            generations,
            arguments,
        } => operations::update_home(&flake, arguments, generations),
        SubCommands::RebuildHome {
            generations,
            arguments,
        } => operations::rebuild_home(arguments, generations),
    }
}
