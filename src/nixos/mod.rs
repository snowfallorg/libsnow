pub mod batch;
pub mod install;
pub mod list;
pub mod rebuild;
pub mod remove;
pub mod update;

#[non_exhaustive]
pub enum AuthMethod<'a> {
    Dbus,
    Sudo,
    Custom(&'a str),
}
