//! The environment variables the app reads: development, testing and
//! measurement switches, never settings a user needs (those are in
//! `SystemSettings`).
//!
//! Each is `FAE_<NAME>`. They were `FFLAME_<NAME>` until 2026-11-26
//! (docs/projects/rename.md), and the old spelling is still read, for one
//! release, so a script or shell profile written before the rename keeps
//! working while it is updated. Build-time ones (`FAE_API_BASE_URL`,
//! `FAE_SKIP_GIT_PROVENANCE`) are read by build.rs, which does the same.

/// `FAE_<name>`, or the old `FFLAME_<name>`; `None` when neither is set
/// (or set to something that is not Unicode).
pub fn var(name: &str) -> Option<String> {
    std::env::var(format!("FAE_{name}")).ok().or_else(|| std::env::var(format!("FFLAME_{name}")).ok())
}

/// As [`var`], for a variable whose presence is the switch.
pub fn var_os(name: &str) -> Option<std::ffi::OsString> {
    std::env::var_os(format!("FAE_{name}")).or_else(|| std::env::var_os(format!("FFLAME_{name}")))
}
