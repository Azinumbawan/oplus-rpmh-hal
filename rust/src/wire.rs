// Raw FFI to libbinder_ndk (the stable C API) and parcel helpers.
//
// The AIDL transport is implemented by hand instead of through generated C++
// code: the interface is fixed (vendor.oplus.hardware.rpmh.IRpmh, version 1,
// hash acf5f9a3960fb9a2f76f86947dff5ca9e0c86e05, three methods numbered 1..3
// plus the reserved pair getInterfaceVersion@0xffffff /
// getInterfaceHash@0xfffffe), so the wire format is fully known and the NDK
// C API is a stable C ABI that Rust can bind directly.
//
// Wire format (matches what the generated ndk backend and the framework's
// Java stub produce):
//   * incoming transactions arrive with the interface token already consumed,
//   * every reply starts with a status header (an int32 exception code, 0 for
//     OK) followed by the out-parameters in declaration order and then the
//     int32 Result when the method returns one,
//   * parcelables are written in the versioned format
//     [int32 1][int32 size][fields], arrays as int32 length + elements,
//     strings as int32 length (UTF-8 bytes, -1 for null) + bytes + NUL.

#![allow(dead_code)]

use std::ffi::{c_char, c_void, CString};
use std::os::raw::c_int;
use std::ptr;

#[repr(C)]
pub struct AIBinder {
    _private: [u8; 0],
}
#[repr(C)]
pub struct AIBinder_Class {
    _private: [u8; 0],
}
#[repr(C)]
pub struct AParcel {
    _private: [u8; 0],
}
#[repr(C)]
pub struct AStatus {
    _private: [u8; 0],
}

pub type BinderStatus = c_int;
pub type TransactionCode = u32;

pub const STATUS_OK: BinderStatus = 0;

pub const TRANSACTION_GET_INTERFACE_VERSION: TransactionCode = 16777215;
pub const TRANSACTION_GET_INTERFACE_HASH: TransactionCode = 16777214;

pub const LOG_LEVEL_INFO: c_int = 4;
pub const LOG_LEVEL_ERROR: c_int = 6;

pub type OnCreate = unsafe extern "C" fn(args: *mut c_void) -> *mut c_void;
pub type OnDestroy = unsafe extern "C" fn(user_data: *mut c_void);
pub type OnTransact = unsafe extern "C" fn(
    binder: *mut AIBinder,
    code: TransactionCode,
    in_parcel: *const AParcel,
    out_parcel: *mut AParcel,
) -> BinderStatus;

extern "C" {
    // android/binder_ibinder.h
    pub fn AIBinder_Class_define(
        interface_descriptor: *const c_char,
        on_create: OnCreate,
        on_destroy: OnDestroy,
        on_transact: OnTransact,
    ) -> *mut AIBinder_Class;
    pub fn AIBinder_new(class: *mut AIBinder_Class, args: *mut c_void) -> *mut AIBinder;
    pub fn AIBinder_getUserData(binder: *mut AIBinder) -> *mut c_void;
    pub fn AIBinder_getCallingUid() -> u32;

    // android/binder_manager.h (platform header)
    pub fn AServiceManager_addService(
        binder: *mut AIBinder,
        instance: *const c_char,
    ) -> BinderStatus;

    // android/binder_process.h (platform header)
    pub fn ABinderProcess_setThreadPoolMaxThreadCount(num_threads: u32) -> bool;
    pub fn ABinderProcess_joinThreadPool();

    // android/binder_parcel.h
    pub fn AParcel_writeInt32(parcel: *mut AParcel, value: i32) -> BinderStatus;
    pub fn AParcel_readInt32(parcel: *const AParcel, value: *mut i32) -> BinderStatus;
    pub fn AParcel_writeInt64(parcel: *mut AParcel, value: i64) -> BinderStatus;
    pub fn AParcel_writeString(
        parcel: *mut AParcel,
        string: *const c_char,
        length: i32,
    ) -> BinderStatus;
    pub fn AParcel_readString(
        parcel: *const AParcel,
        string_data: *mut c_void,
        allocator: Option<unsafe extern "C" fn(*mut c_void, i32, *mut *mut c_char) -> bool>,
    ) -> BinderStatus;
    pub fn AParcel_writeStatusHeader(parcel: *mut AParcel, status: *const AStatus) -> BinderStatus;
    pub fn AParcel_getDataPosition(parcel: *const AParcel) -> i32;
    pub fn AParcel_setDataPosition(parcel: *const AParcel, position: i32) -> BinderStatus;

    // android/binder_status.h
    pub fn AStatus_newOk() -> *mut AStatus;
    pub fn AStatus_delete(status: *mut AStatus);

    // android/log.h
    pub fn __android_log_print(prio: c_int, tag: *const c_char, fmt: *const c_char, ...) -> c_int;
}

pub fn log(level: c_int, tag: &str, msg: &str) {
    let tag = match CString::new(tag) {
        Ok(t) => t,
        Err(_) => return,
    };
    let fmt = CString::new("%s").unwrap();
    let msg = match CString::new(msg) {
        Ok(m) => m,
        Err(_) => return,
    };
    // SAFETY: all three strings are valid NUL-terminated C strings.
    unsafe {
        __android_log_print(level, tag.as_ptr(), fmt.as_ptr(), msg.as_ptr());
    }
}

// Versioned-parcelable wrapper: [int32 1][int32 size][fields]. The size
// includes the size field itself and is patched after the fields are out.
// Both directions honour it: a missing marker or size on write makes the
// generated client treat the first field as the marker and fail.

pub fn get_pos(parcel: *const AParcel) -> i32 {
    // SAFETY: read-only query of a valid parcel.
    unsafe { AParcel_getDataPosition(parcel) }
}

pub fn set_pos(parcel: *const AParcel, pos: i32) -> Result<(), BinderStatus> {
    // SAFETY: positions used here come from get_pos on the same parcel.
    let rc = unsafe { AParcel_setDataPosition(parcel, pos) };
    if rc == STATUS_OK {
        Ok(())
    } else {
        Err(rc)
    }
}

pub fn begin_parcelable(out: *mut AParcel) -> Result<i32, BinderStatus> {
    write_i32(out, 1)?;
    let start = get_pos(out);
    write_i32(out, 0)?;
    Ok(start)
}

pub fn end_parcelable(out: *mut AParcel, start: i32) -> Result<(), BinderStatus> {
    let end = get_pos(out);
    set_pos(out, start)?;
    write_i32(out, end - start)?;
    set_pos(out, end)
}

// Write helpers.

pub fn write_ok_status(out: *mut AParcel) -> Result<(), BinderStatus> {
    // SAFETY: out is a valid reply parcel from onTransact.
    let status = unsafe { AStatus_newOk() };
    if status.is_null() {
        return Err(STATUS_OK - 1);
    }
    let rc = unsafe { AParcel_writeStatusHeader(out, status) };
    unsafe { AStatus_delete(status) };
    if rc == STATUS_OK {
        Ok(())
    } else {
        Err(rc)
    }
}

pub fn write_i32(out: *mut AParcel, value: i32) -> Result<(), BinderStatus> {
    // SAFETY: out is a valid reply parcel.
    let rc = unsafe { AParcel_writeInt32(out, value) };
    if rc == STATUS_OK {
        Ok(())
    } else {
        Err(rc)
    }
}

pub fn write_i64(out: *mut AParcel, value: i64) -> Result<(), BinderStatus> {
    // SAFETY: out is a valid reply parcel.
    let rc = unsafe { AParcel_writeInt64(out, value) };
    if rc == STATUS_OK {
        Ok(())
    } else {
        Err(rc)
    }
}

pub fn write_bool(out: *mut AParcel, value: bool) -> Result<(), BinderStatus> {
    // AIDL booleans write as int32 0/1 through the NDK API's writeBool, which
    // this binding does not declare separately; the generated backend's
    // writeBool is exactly AParcel_writeInt32(0|1).
    write_i32(out, if value { 1 } else { 0 })
}

pub fn write_string(out: *mut AParcel, value: &str) -> Result<(), BinderStatus> {
    let cstr = match CString::new(value) {
        Ok(c) => c,
        Err(_) => return Err(-22), // STATUS_BAD_VALUE
    };
    // SAFETY: out is a valid reply parcel; cstr is NUL-terminated and its
    // length is passed explicitly as the UTF-8 byte count.
    let rc = unsafe { AParcel_writeString(out, cstr.as_ptr(), cstr.as_bytes().len() as i32) };
    if rc == STATUS_OK {
        Ok(())
    } else {
        Err(rc)
    }
}

// Read helpers.

pub fn read_i32(in_parcel: *const AParcel) -> Result<i32, BinderStatus> {
    let mut value: i32 = 0;
    // SAFETY: in_parcel is a valid incoming data parcel; &mut value is a
    // writable i32.
    let rc = unsafe { AParcel_readInt32(in_parcel, &mut value) };
    if rc == STATUS_OK {
        Ok(value)
    } else {
        Err(rc)
    }
}

pub fn read_string(in_parcel: *const AParcel) -> Result<String, BinderStatus> {
    let mut ctx = StringCtx {
        buf: ptr::null_mut(),
        layout: std::alloc::Layout::new::<u8>(),
    };
    // SAFETY: in_parcel is a valid incoming data parcel. The allocator
    // callback leases a buffer that libbinder fills with the string, and the
    // StringCtx Drop frees it when the ctx goes out of scope.
    let rc = unsafe {
        AParcel_readString(
            in_parcel,
            &mut ctx as *mut StringCtx as *mut c_void,
            Some(string_allocator),
        )
    };
    if rc != STATUS_OK {
        return Err(rc);
    }
    if ctx.buf.is_null() {
        // A null string in the parcel reads as empty.
        return Ok(String::new());
    }
    // SAFETY: buf holds layout.size() bytes with the string and its NUL
    // terminator written by libbinder inside them.
    let bytes = unsafe { std::slice::from_raw_parts(ctx.buf, ctx.layout.size()) };
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    Ok(String::from_utf8_lossy(&bytes[..end]).into_owned())
}

/// A binder transaction is capped at 1 MiB, so no larger string is legitimate.
const MAX_PARCEL_STRING: usize = 1 << 20;

struct StringCtx {
    buf: *mut u8,
    layout: std::alloc::Layout,
}

impl Drop for StringCtx {
    fn drop(&mut self) {
        if !self.buf.is_null() {
            // SAFETY: buf was allocated with exactly this layout.
            unsafe { std::alloc::dealloc(self.buf, self.layout) };
        }
    }
}

unsafe extern "C" fn string_allocator(
    data: *mut c_void,
    length: i32,
    buffer: *mut *mut c_char,
) -> bool {
    let ctx = &mut *(data as *mut StringCtx);
    // AParcel_stringAllocator contract (binder_parcel.h): length includes the
    // NUL terminator, and -1 means a null string, which succeeds with no
    // buffer. 0 is documented as never sent; treat it the same way.
    if length <= 0 {
        return true;
    }
    if (length as usize) > MAX_PARCEL_STRING {
        return false;
    }
    let layout = match std::alloc::Layout::from_size_align(length as usize, 1) {
        Ok(l) => l,
        Err(_) => return false,
    };
    let p = std::alloc::alloc(layout);
    if p.is_null() {
        return false;
    }
    ctx.buf = p;
    ctx.layout = layout;
    *buffer = p as *mut c_char;
    true
}

/// Writes an int32 list length followed by nothing: an unreadable set is
/// answered with a length of 0, which the framework accepts.
pub fn write_empty_i32_list(out: *mut AParcel) -> Result<(), BinderStatus> {
    write_i32(out, 0)
}
