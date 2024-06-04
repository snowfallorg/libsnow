pub mod list;
pub mod update;
pub mod install;
pub mod remove;
pub mod rebuild;

pub enum AuthMethod <'a> {
    Pkexec,
    Sudo,
    Custom(&'a str),
}
