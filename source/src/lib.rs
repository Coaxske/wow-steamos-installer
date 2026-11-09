pub mod addons;
pub mod catalog;
mod developer_tools;
#[cfg(any(test, feature = "developer-tools"))]
pub mod device_tests;
pub mod discovery;
pub mod engine;
pub mod folder_links;
pub mod fsutil;
pub mod merge;
pub mod model;
pub mod products;
pub mod release;
mod state_merge;
pub mod steam;
mod steam_process;
pub mod transaction;
pub mod updater;
pub mod vdf;
pub mod workflow;

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::atomic::{AtomicBool, Ordering};
pub static CANCEL: AtomicBool = AtomicBool::new(false);
pub type Result<T> = std::result::Result<T, String>;

pub fn cancelled() -> Result<()> {
    if CANCEL.load(Ordering::Relaxed) {
        Err("Cancelled. No new operation will start.".into())
    } else {
        Ok(())
    }
}

/// The Qt adapter passes a NUL-terminated UTF-8 request. Rust owns the response
/// until wow_free is called. A panic never unwinds into C++.
///
/// # Safety
/// `request` must be null or point to a readable NUL-terminated UTF-8 buffer
/// that remains valid for this synchronous call. Free the result exactly once
/// using `wow_free`. Ordinary setup requests are serialized; the separately locked updater uses its own cancellation state.
#[no_mangle]
pub unsafe extern "C" fn wow_request(request: *const c_char) -> *mut c_char {
    let result = std::panic::catch_unwind(|| {
        if request.is_null() {
            return Err("Missing request".into());
        }
        let input = CStr::from_ptr(request)
            .to_str()
            .map_err(|_| "Invalid UTF-8")?;
        let req = serde_json::from_str(input).map_err(|e| format!("Invalid request: {e}"))?;
        engine::dispatch(req)
    });
    let value = match result {
        Ok(Ok(v)) => serde_json::json!({"ok":true,"data":v}),
        Ok(Err(e)) => serde_json::json!({"ok":false,"error":e}),
        Err(_) => {
            serde_json::json!({"ok":false,"error":"Unexpected internal error. Restart setup to inspect saved progress."})
        }
    };
    CString::new(value.to_string())
        .expect("JSON escapes NUL")
        .into_raw()
}

#[no_mangle]
/// # Safety
/// `value` must be null or a pointer returned by `wow_request` that has not
/// already been freed. No other allocator may release it.
pub unsafe extern "C" fn wow_free(value: *mut c_char) {
    if !value.is_null() {
        drop(CString::from_raw(value));
    }
}
#[no_mangle]
pub extern "C" fn wow_update_cancel() -> i32 {
    i32::from(updater::cancel())
}
#[no_mangle]
pub extern "C" fn wow_update_reset_cancel() {
    updater::reset_cancel();
}

#[no_mangle]
pub extern "C" fn wow_cancel() {
    CANCEL.store(true, Ordering::Relaxed);
}
#[no_mangle]
pub extern "C" fn wow_reset_cancel() {
    CANCEL.store(false, Ordering::Relaxed);
}

#[no_mangle]
pub extern "C" fn wow_version() -> *const c_char {
    concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr().cast()
}

fn steam_state_code(result: Result<bool>) -> i32 {
    match result {
        Ok(false) => 0,
        Ok(true) => 1,
        Err(_) => -1,
    }
}
/// Shared Qt/engine state: -1 unknown, 0 this user's Steam stopped, 1 running.
#[no_mangle]
pub extern "C" fn wow_steam_state() -> i32 {
    std::panic::catch_unwind(steam_process::current_user_running)
        .map(steam_state_code)
        .unwrap_or(-1)
}

/// Sanitized opt-in controller handoff metadata; no command lines or account data.
#[no_mangle]
#[cfg(any(test, feature = "developer-tools"))]
pub extern "C" fn wow_steam_snapshot() -> *mut std::os::raw::c_char {
    let value = std::panic::catch_unwind(steam_process::diagnostic_snapshot)
        .unwrap_or_else(|_| serde_json::json!({"snapshot_error":true}));
    std::ffi::CString::new(value.to_string())
        .unwrap()
        .into_raw()
}

#[no_mangle]
pub extern "C" fn wow_source() -> *const c_char {
    concat!(env!("WOW_SOURCE_SHA256"), "\0").as_ptr().cast()
}

/// Read-only revalidation for the installer-owned desktop helper. No operation
/// lock is reacquired: the parent holds it throughout the reviewed operation.
/// # Safety
/// `request` must remain a valid NUL-terminated UTF-8 string for this call.
#[no_mangle]
#[cfg(any(test, feature = "developer-tools"))]
pub unsafe extern "C" fn wow_controller_validate(request: *const c_char) -> i32 {
    std::panic::catch_unwind(|| {
        if request.is_null() {
            return false;
        }
        let text = match CStr::from_ptr(request).to_str() {
            Ok(s) => s,
            Err(_) => return false,
        };
        let value = match serde_json::from_str(text) {
            Ok(v) => v,
            Err(_) => return false,
        };
        engine::validate_controller_target(value).is_ok()
    })
    .map(i32::from)
    .unwrap_or(0)
}

/// Validate the current registration capability without reading private manager data.
///
/// # Safety
/// `request` must be null or point to a valid NUL-terminated UTF-8 string for this call.
#[no_mangle]
#[cfg(any(test, feature = "developer-tools"))]
pub unsafe extern "C" fn wow_addon_registration_validate(request: *const c_char) -> i32 {
    std::panic::catch_unwind(|| {
        if request.is_null() {
            return false;
        }
        let Ok(text) = CStr::from_ptr(request).to_str() else {
            return false;
        };
        let Ok(value) = serde_json::from_str(text) else {
            return false;
        };
        engine::validate_addon_registration(value).is_ok()
    })
    .map(i32::from)
    .unwrap_or(0)
}

/// Validate the current lifecycle capability without reading private manager data.
///
/// # Safety
/// `request` must be null or point to a valid NUL-terminated UTF-8 string for this call.
#[no_mangle]
#[cfg(any(test, feature = "developer-tools"))]
pub unsafe extern "C" fn wow_addon_lifecycle_validate(request: *const c_char) -> i32 {
    std::panic::catch_unwind(|| {
        if request.is_null() {
            return false;
        }
        let Ok(text) = CStr::from_ptr(request).to_str() else {
            return false;
        };
        let Ok(value) = serde_json::from_str(text) else {
            return false;
        };
        engine::validate_addon_lifecycle(value).is_ok()
    })
    .map(i32::from)
    .unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn wow_developer_tools() -> i32 {
    i32::from(cfg!(feature = "developer-tools"))
}
