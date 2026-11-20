// Donor V1 ordering: status, return Result, then the out state/list.

use crate::wire::*;
pub use rpmh_service::protocol::{DESCRIPTOR, INTERFACE_HASH, INTERFACE_VERSION};
use rpmh_service::{
    core::{self, Statistics},
    io_shim,
    protocol::{self, ReplyWriter},
};
use std::ffi::c_void;
pub const LOG_TAG: &str = "OplusRpmhHAL";

pub unsafe extern "C" fn on_create(_args: *mut c_void) -> *mut c_void {
    let hardware = io_shim::property_get("ro.hardware").unwrap_or_default();
    Box::into_raw(Box::new(Statistics::new(core::verified_source(&hardware)))).cast()
}
pub unsafe extern "C" fn on_destroy(data: *mut c_void) {
    if !data.is_null() {
        drop(Box::from_raw(data.cast::<Statistics>()));
    }
}

enum Args {
    None,
    Name(String),
    Dump(i32),
}
fn decode(code: u32, input: *const AParcel) -> Result<Args, i32> {
    let args = match code {
        1 => Args::Name(read_string(input)?),
        2 | TRANSACTION_GET_INTERFACE_VERSION | TRANSACTION_GET_INTERFACE_HASH => Args::None,
        3 => Args::Dump(read_i32(input)?),
        _ => return Err(STATUS_UNKNOWN_TRANSACTION),
    };
    enforce_no_data_avail(input)?;
    Ok(args)
}
pub unsafe extern "C" fn on_transact(
    binder: *mut AIBinder,
    code: u32,
    input: *const AParcel,
    output: *mut AParcel,
) -> i32 {
    dispatch(binder, code, input, output)
        .err()
        .unwrap_or(STATUS_OK)
}
unsafe fn dispatch(
    binder: *mut AIBinder,
    code: u32,
    input: *const AParcel,
    out: *mut AParcel,
) -> Result<(), i32> {
    let args = decode(code, input)?;
    match (code, args) {
        (TRANSACTION_GET_INTERFACE_VERSION, Args::None) => {
            write_ok_status(out)?;
            write_i32(out, INTERFACE_VERSION)
        }
        (TRANSACTION_GET_INTERFACE_HASH, Args::None) => {
            write_ok_status(out)?;
            write_string(out, INTERFACE_HASH)
        }
        (3, Args::Dump(kind)) => {
            write_ok_status(out)?;
            write_i32(out, core::trigger_dump_type(kind))
        }
        (code, args) => {
            let data = AIBinder_getUserData(binder);
            if data.is_null() {
                return Err(STATUS_UNEXPECTED_NULL);
            }
            let statistics = &*data.cast::<Statistics>();
            let uid = AIBinder_getCallingUid();
            match (code, args) {
                (1, Args::Name(name)) => {
                    let (result, state) = match statistics.get(uid, &name) {
                        Ok(state) => (core::RESULT_OK, state),
                        Err(error) => (error.result(), Default::default()),
                    };
                    protocol::single(&mut ParcelWriter(out), result, &state)
                }
                (2, Args::None) => {
                    let (result, states) = match statistics.list(uid) {
                        Ok(states) => (core::RESULT_OK, states),
                        Err(error) => (error.result(), Vec::new()),
                    };
                    protocol::list(&mut ParcelWriter(out), result, &states)
                }
                _ => Err(STATUS_BAD_VALUE),
            }
        }
    }
}

struct ParcelWriter(*mut AParcel);
impl ReplyWriter for ParcelWriter {
    type Error = i32;
    fn invalid_value(&self) -> i32 {
        STATUS_BAD_VALUE
    }
    fn ok_status(&mut self) -> Result<(), i32> {
        write_ok_status(self.0)
    }
    fn int32(&mut self, value: i32) -> Result<(), i32> {
        write_i32(self.0, value)
    }
    fn int64(&mut self, value: i64) -> Result<(), i32> {
        write_i64(self.0, value)
    }
    fn string(&mut self, value: &str) -> Result<(), i32> {
        write_string(self.0, value)
    }
    fn begin_state(&mut self) -> Result<i32, i32> {
        begin_parcelable(self.0)
    }
    fn end_state(&mut self, start: i32) -> Result<(), i32> {
        end_parcelable(self.0, start)
    }
}
