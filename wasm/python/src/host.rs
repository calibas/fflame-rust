//! The two functions the host (Python) provides, and the module's side
//! of the call: a JSON command, optionally a block of bytes beside it
//! (a buffer write, a texture upload), and back a result -- JSON or
//! bytes, as the command says.

use serde_json::Value;

#[link(wasm_import_module = "fflame_gpu")]
extern "C" {
    /// Runs one command; returns the length of its result (0: none).
    fn call(cmd: *const u8, cmd_len: usize, data: *const u8, data_len: usize) -> usize;
    /// Copies the last command's result to `dst`.
    fn take(dst: *mut u8);
}

/// A command with bytes beside it; its result's bytes.
pub fn call_with(cmd: &Value, data: &[u8]) -> Vec<u8> {
    let text = serde_json::to_vec(cmd).expect("a command serializes");
    let n = unsafe { call(text.as_ptr(), text.len(), data.as_ptr(), data.len()) };
    let mut out = vec![0u8; n];
    if n > 0 {
        unsafe { take(out.as_mut_ptr()) };
    }
    out
}

/// A command whose result is ignored.
pub fn send(cmd: &Value) {
    call_with(cmd, &[]);
}

/// A command whose result is JSON.
pub fn query(cmd: &Value) -> Value {
    let out = call_with(cmd, &[]);
    serde_json::from_slice(&out).expect("the host answers in JSON")
}
