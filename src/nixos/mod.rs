pub mod install;
pub mod list;
pub mod rebuild;
pub mod remove;
pub mod update;

pub enum AuthMethod<'a> {
    Pkexec,
    Sudo,
    Custom(&'a str),
}
