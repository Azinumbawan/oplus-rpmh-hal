// Real NDK APIs perform interface-token and UTF-16 transport handling.
#![allow(dead_code)]

use rpmh_service::protocol::{consumed, parcelable_size, string_length, MAX_STRING_BYTES};
pub use rpmh_service::protocol::{
    STATUS_BAD_VALUE, STATUS_UNEXPECTED_NULL, STATUS_UNKNOWN_TRANSACTION,
};
use std::ffi::{c_char, c_void, CString};

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
pub type BinderStatus = i32;
pub type TransactionCode = u32;
pub const STATUS_OK: i32 = 0;
pub const STATUS_NO_MEMORY: i32 = -12;
pub const TRANSACTION_GET_INTERFACE_VERSION: u32 = 0xffffff;
pub const TRANSACTION_GET_INTERFACE_HASH: u32 = 0xfffffe;
pub const LOG_LEVEL_INFO: i32 = 4;
pub const LOG_LEVEL_ERROR: i32 = 6;
pub type OnCreate = unsafe extern "C" fn(*mut c_void) -> *mut c_void;
pub type OnDestroy = unsafe extern "C" fn(*mut c_void);
pub type OnTransact = unsafe extern "C" fn(*mut AIBinder, u32, *const AParcel, *mut AParcel) -> i32;

extern "C" {
    pub fn AIBinder_Class_define(
        descriptor: *const c_char,
        create: OnCreate,
        destroy: OnDestroy,
        transact: OnTransact,
    ) -> *mut AIBinder_Class;
    pub fn AIBinder_new(class: *const AIBinder_Class, args: *mut c_void) -> *mut AIBinder;
    pub fn AIBinder_getUserData(binder: *mut AIBinder) -> *mut c_void;
    pub fn AIBinder_getCallingUid() -> u32;
    pub fn AIBinder_markVintfStability(binder: *mut AIBinder);
    pub fn AServiceManager_addService(binder: *mut AIBinder, instance: *const c_char) -> i32;
    pub fn ABinderProcess_setThreadPoolMaxThreadCount(count: u32) -> bool;
    pub fn ABinderProcess_joinThreadPool();
    fn AParcel_writeInt32(parcel: *mut AParcel, value: i32) -> i32;
    fn AParcel_readInt32(parcel: *const AParcel, value: *mut i32) -> i32;
    fn AParcel_writeInt64(parcel: *mut AParcel, value: i64) -> i32;
    fn AParcel_writeString(parcel: *mut AParcel, value: *const c_char, length: i32) -> i32;
    fn AParcel_readString(
        parcel: *const AParcel,
        data: *mut c_void,
        allocator: Option<unsafe extern "C" fn(*mut c_void, i32, *mut *mut c_char) -> bool>,
    ) -> i32;
    fn AParcel_getDataPosition(parcel: *const AParcel) -> i32;
    fn AParcel_getDataSize(parcel: *const AParcel) -> i32;
    fn AParcel_setDataPosition(parcel: *const AParcel, position: i32) -> i32;
    fn AParcel_writeStatusHeader(parcel: *mut AParcel, status: *const AStatus) -> i32;
    fn AStatus_newOk() -> *mut AStatus;
    fn AStatus_delete(status: *mut AStatus);
    fn __android_log_print(priority: i32, tag: *const c_char, format: *const c_char, ...) -> i32;
}

fn check(status: i32) -> Result<(), i32> {
    if status == 0 {
        Ok(())
    } else {
        Err(status)
    }
}
pub fn log(priority: i32, tag: &str, message: &str) {
    if let (Ok(tag), Ok(message)) = (CString::new(tag), CString::new(message)) {
        unsafe {
            __android_log_print(priority, tag.as_ptr(), c"%s".as_ptr(), message.as_ptr());
        }
    }
}
pub fn enforce_no_data_avail(parcel: *const AParcel) -> Result<(), i32> {
    consumed(unsafe { AParcel_getDataPosition(parcel) }, unsafe {
        AParcel_getDataSize(parcel)
    })
}
pub fn write_ok_status(out: *mut AParcel) -> Result<(), i32> {
    let status = unsafe { AStatus_newOk() };
    if status.is_null() {
        return Err(STATUS_NO_MEMORY);
    }
    let result = unsafe { AParcel_writeStatusHeader(out, status) };
    unsafe { AStatus_delete(status) };
    check(result)
}
pub fn write_i32(out: *mut AParcel, value: i32) -> Result<(), i32> {
    check(unsafe { AParcel_writeInt32(out, value) })
}
pub fn write_i64(out: *mut AParcel, value: i64) -> Result<(), i32> {
    check(unsafe { AParcel_writeInt64(out, value) })
}
pub fn write_string(out: *mut AParcel, value: &str) -> Result<(), i32> {
    if value.len() > MAX_STRING_BYTES {
        return Err(STATUS_BAD_VALUE);
    }
    let value = CString::new(value).map_err(|_| STATUS_BAD_VALUE)?;
    check(unsafe { AParcel_writeString(out, value.as_ptr(), value.as_bytes().len() as i32) })
}
pub fn read_i32(parcel: *const AParcel) -> Result<i32, i32> {
    let mut value = 0;
    check(unsafe { AParcel_readInt32(parcel, &mut value) })?;
    Ok(value)
}

pub fn begin_parcelable(out: *mut AParcel) -> Result<i32, i32> {
    write_i32(out, 1)?;
    let start = unsafe { AParcel_getDataPosition(out) };
    if start < 0 {
        return Err(STATUS_BAD_VALUE);
    }
    write_i32(out, 0)?;
    Ok(start)
}
pub fn end_parcelable(out: *mut AParcel, start: i32) -> Result<(), i32> {
    let end = unsafe { AParcel_getDataPosition(out) };
    let size = parcelable_size(start, end)?;
    check(unsafe { AParcel_setDataPosition(out, start) })?;
    write_i32(out, size)?;
    check(unsafe { AParcel_setDataPosition(out, end) })
}

#[derive(Default)]
struct StringCtx {
    bytes: Vec<u8>,
    null: bool,
    error: Option<i32>,
}
unsafe extern "C" fn string_allocator(
    data: *mut c_void,
    length: i32,
    out: *mut *mut c_char,
) -> bool {
    let ctx = &mut *data.cast::<StringCtx>();
    match string_length(length) {
        Ok(None) => {
            ctx.null = true;
            // Null values do not provide an output-buffer pointer.
            true
        }
        Ok(Some(length)) => {
            if out.is_null() {
                ctx.error = Some(STATUS_BAD_VALUE);
                return false;
            }
            if ctx.bytes.try_reserve_exact(length).is_err() {
                ctx.error = Some(STATUS_NO_MEMORY);
                return false;
            }
            ctx.bytes.resize(length, 0);
            *out = ctx.bytes.as_mut_ptr().cast();
            true
        }
        Err(error) => {
            ctx.error = Some(error);
            false
        }
    }
}
pub fn read_string(parcel: *const AParcel) -> Result<String, i32> {
    let mut ctx = StringCtx::default();
    let status = unsafe {
        AParcel_readString(
            parcel,
            (&mut ctx as *mut StringCtx).cast(),
            Some(string_allocator),
        )
    };
    if let Some(error) = ctx.error {
        return Err(error);
    }
    check(status)?;
    if ctx.null {
        return Err(STATUS_UNEXPECTED_NULL);
    }
    if ctx.bytes.pop() != Some(0) || ctx.bytes.contains(&0) {
        return Err(STATUS_BAD_VALUE);
    }
    String::from_utf8(ctx.bytes).map_err(|_| STATUS_BAD_VALUE)
}
