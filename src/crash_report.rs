//! What the app was doing when it died (desktop).
//!
//! A Rust panic is logged by the panic hook (`main.rs`), but a fault in
//! native code -- the GPU driver compiling a shader, say -- never reaches
//! Rust: the process exits with a code like `0xc000041d` and nothing in
//! `crash.log`. On Windows a **vectored exception handler** sees the fault
//! first, so this one appends to `crash.log` the exception's code, the
//! module and offset it happened in, and the **breadcrumbs**: what the
//! viewport last loaded, and what was rendering (a preview, an export, a
//! frame) when it happened. It then lets the exception go on as before.
//!
//! It logs at the first chance, before anything could handle the
//! exception, so a line can name a fault a driver handled itself. Each
//! line says so, only the fatal kinds are logged (access violation,
//! illegal instruction, stack overflow, fail-fast, an exception in a
//! window callback), and at most [`MAX_LINES`] a session.

use std::sync::Mutex;

/// What the viewport last loaded.
static VIEWPORT: Mutex<String> = Mutex::new(String::new());
/// What `renderer::render` last started (a preview, an export, a
/// texture): the offscreen work, which the viewport's frames are not.
static ACTIVITY: Mutex<String> = Mutex::new(String::new());

/// Record what the viewport shows now.
pub fn viewport(what: impl Into<String>) {
    if let Ok(mut v) = VIEWPORT.lock() {
        *v = what.into();
    }
}

/// Record what is rendering now.
pub fn activity(what: impl Into<String>) {
    if let Ok(mut a) = ACTIVITY.lock() {
        *a = what.into();
    }
}

/// A config as a breadcrumb names it: its name, mode, and for an escape
/// config its formula and colouring.
pub fn describe(config: &crate::config::FractalConfig) -> String {
    use crate::scene::transforms::RenderMode;
    let mode = format!("{:?}", config.render_mode);
    match config.render_mode {
        RenderMode::Escape => format!(
            "\"{}\" ({mode}, formula {}, colouring {})",
            config.flame.name, config.escape.formula, config.escape.coloring
        ),
        RenderMode::Simulation => format!("\"{}\" ({mode}, model {})", config.flame.name, config.sim.model),
        _ => format!("\"{}\" ({mode})", config.flame.name),
    }
}

/// The breadcrumbs, as a log line's tail. `try_lock`: the faulting thread
/// may hold a lock.
fn breadcrumbs() -> String {
    let read = |m: &Mutex<String>| m.try_lock().map(|s| s.clone()).unwrap_or_else(|_| "(busy)".into());
    format!("viewport: {}; last offscreen render: {}", read(&VIEWPORT), read(&ACTIVITY))
}

/// The most lines a session writes.
pub const MAX_LINES: usize = 10;

/// Install the handler (Windows; elsewhere nothing). Call once, early.
pub fn install() {
    #[cfg(windows)]
    windows_handler::install();
}

#[cfg(windows)]
mod windows_handler {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use windows_sys::Win32::Foundation::HMODULE;
    use windows_sys::Win32::System::Diagnostics::Debug::{AddVectoredExceptionHandler, EXCEPTION_POINTERS};
    use windows_sys::Win32::System::LibraryLoader::{
        GetModuleFileNameW, GetModuleHandleExW, GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentThreadId, GetCurrentThreadStackLimits};

    static LINES: AtomicUsize = AtomicUsize::new(0);
    /// The thread `install` ran on: the main one.
    static MAIN_THREAD: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

    /// Which thread faulted, and its stack's size.
    fn thread_line() -> String {
        let id = unsafe { GetCurrentThreadId() };
        let (mut low, mut high) = (0usize, 0usize);
        unsafe { GetCurrentThreadStackLimits(&mut low, &mut high) };
        let which = if id == MAIN_THREAD.load(Ordering::Relaxed) { "the main thread" } else { "another thread" };
        format!("on {which} ({id}), stack {} KB", high.saturating_sub(low) / 1024)
    }
    const EXCEPTION_CONTINUE_SEARCH: i32 = 0;

    /// The fatal kinds, and their names.
    fn name(code: u32) -> Option<&'static str> {
        Some(match code {
            0xC000_0005 => "access violation",
            0xC000_001D => "illegal instruction",
            0xC000_0096 => "privileged instruction",
            0xC000_00FD => "stack overflow",
            0xC000_0409 => "fail-fast (stack buffer overrun)",
            0xC000_041D => "exception in a window callback",
            _ => return None,
        })
    }

    /// The module holding `addr`, and the offset into it.
    unsafe fn module_of(addr: usize) -> (String, usize) {
        let mut module: HMODULE = std::ptr::null_mut();
        let flags = GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT;
        if unsafe { GetModuleHandleExW(flags, addr as *const u16, &mut module) } == 0 || module.is_null() {
            return ("(unknown module)".into(), addr);
        }
        let mut buf = [0u16; 260];
        let n = unsafe { GetModuleFileNameW(module, buf.as_mut_ptr(), buf.len() as u32) } as usize;
        let path = String::from_utf16_lossy(&buf[..n.min(buf.len())]);
        let file = path.rsplit(['\\', '/']).next().unwrap_or(&path).to_string();
        (file, addr.wrapping_sub(module as usize))
    }

    unsafe extern "system" fn handler(info: *mut EXCEPTION_POINTERS) -> i32 {
        let record = unsafe { info.as_ref().and_then(|i| i.ExceptionRecord.as_ref()) };
        let Some(record) = record else { return EXCEPTION_CONTINUE_SEARCH };
        let code = record.ExceptionCode as u32;
        let Some(kind) = name(code) else { return EXCEPTION_CONTINUE_SEARCH };
        if LINES.fetch_add(1, Ordering::Relaxed) >= super::MAX_LINES {
            return EXCEPTION_CONTINUE_SEARCH;
        }
        let (module, offset) = unsafe { module_of(record.ExceptionAddress as usize) };
        let line = format!(
            "native exception 0x{code:08X} ({kind}) in {module}+0x{offset:X} {} (logged at first chance: fatal unless something handles it); {}",
            thread_line(),
            super::breadcrumbs()
        );
        append(&line);
        EXCEPTION_CONTINUE_SEARCH
    }

    fn append(line: &str) {
        use std::io::Write;
        if let Ok(dir) = crate::storage::backend::get_app_data_dir() {
            let stamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("crash.log")) {
                let _ = writeln!(f, "[{stamp}] {line}");
            }
        }
    }

    pub fn install() {
        MAIN_THREAD.store(unsafe { GetCurrentThreadId() }, Ordering::Relaxed);
        // First in the chain: before any other handler can swallow it.
        unsafe {
            AddVectoredExceptionHandler(1, Some(handler));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_breadcrumbs_say_what_was_on() {
        let mut c = crate::config::FractalConfig::default();
        c.render_mode = crate::scene::transforms::RenderMode::Escape;
        c.escape.formula = "quaternion_julia_solid".into();
        c.flame.name = "Quaternion Julia".into();
        let d = describe(&c);
        assert!(d.contains("Quaternion Julia") && d.contains("quaternion_julia_solid"), "{d}");
        viewport(d.clone());
        activity("preview of x");
        let b = breadcrumbs();
        assert!(b.contains("Quaternion Julia") && b.contains("preview of x"), "{b}");
    }
}
