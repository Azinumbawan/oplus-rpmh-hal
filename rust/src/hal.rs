// The Rpmh HAL service: the onTransact dispatch for the three IRpmh methods
// plus the reserved getInterfaceVersion / getInterfaceHash pair.
//
// Every handler keeps the shipped wire contract:
//   * the binder status is always OK; failure is reported through the int32
//     Result out-parameter (Result::UN_SUPPORTED / PERMISSION_DENIED),
//   * a caller at or above uid 1002 is refused before any work,
//   * the out parcelable is written even when the parse fails, as the
//     generated client reads it unconditionally before the Result,
//   * the log tag is "vendor.oplus.hardware.rpmh-V1-service".

use std::ffi::c_void;

use crate::wire::*;

use rpmh_service::core::{
    self, select_parser, PowerStateSubsystemSleepState, MAX_ALLOWED_UID_EXCLUSIVE,
    RESULT_OK, RESULT_PERMISSION_DENIED, RESULT_UN_SUPPORTED,
};
use rpmh_service::io_shim;

pub const LOG_TAG: &str = "vendor.oplus.hardware.rpmh-V1-service";

pub const DESCRIPTOR: &str = "vendor.oplus.hardware.rpmh.IRpmh";
pub const INTERFACE_HASH: &str = "acf5f9a3960fb9a2f76f86947dff5ca9e0c86e05";
pub const INTERFACE_VERSION: i32 = 1;

/// The service state: the platform parser, built once from ro.hardware.
pub struct Service {
    parser: Box<dyn core::SubsystemStateParser>,
}

impl Service {
    pub fn new() -> Self {
        let hardware = io_shim::property_get("ro.hardware").unwrap_or_default();
        Service {
            parser: select_parser(&hardware),
        }
    }
}

pub unsafe extern "C" fn on_create(_args: *mut c_void) -> *mut c_void {
    let svc = Box::new(Service::new());
    Box::into_raw(svc) as *mut c_void
}

pub unsafe extern "C" fn on_destroy(user_data: *mut c_void) {
    if !user_data.is_null() {
        drop(Box::from_raw(user_data as *mut Service));
    }
}

pub unsafe extern "C" fn on_transact(
    binder: *mut AIBinder,
    code: TransactionCode,
    in_parcel: *const AParcel,
    out_parcel: *mut AParcel,
) -> BinderStatus {
    let res = match code {
        1 => {
            let svc = service_of(binder);
            match read_string(in_parcel) {
                Ok(name) => {
                    let mut state = PowerStateSubsystemSleepState::default();
                    let result = if uid_allowed() {
                        match svc.parser.parse_subsystem_sleep_state(&name) {
                            Some(parsed) => {
                                state = parsed;
                                RESULT_OK
                            }
                            None => RESULT_UN_SUPPORTED,
                        }
                    } else {
                        log_denied_uid();
                        RESULT_PERMISSION_DENIED
                    };
                    write_get_single(out_parcel, &state, result)
                }
                Err(rc) => Err(rc),
            }
        }
        2 => {
            let svc = service_of(binder);
            let mut list = Vec::new();
            let result = if uid_allowed() {
                for name in core::SUBSYSTEMS {
                    if let Some(state) = svc.parser.parse_subsystem_sleep_state(name) {
                        list.push(state);
                    }
                }
                RESULT_OK
            } else {
                log_denied_uid();
                RESULT_PERMISSION_DENIED
            };
            write_get_list(out_parcel, &list, result)
        }
        3 => match read_i32(in_parcel) {
            Ok(dump_type) => {
                let result = if uid_allowed() {
                    core::trigger_dump_type(dump_type)
                } else {
                    log_denied_uid();
                    RESULT_PERMISSION_DENIED
                };
                write_trigger_dump(out_parcel, result)
            }
            Err(rc) => Err(rc),
        },
        TRANSACTION_GET_INTERFACE_VERSION => i32_reply(out_parcel, INTERFACE_VERSION),
        TRANSACTION_GET_INTERFACE_HASH => string_reply(out_parcel, INTERFACE_HASH.to_string()),
        _ => {
            return -31; // STATUS_UNKNOWN_TRANSACTION
        }
    };

    match res {
        Ok(()) => STATUS_OK,
        Err(rc) => rc,
    }
}

/// The shipped service holds no per-binder state, so the single instance is
/// reachable through AIBinder_getUserData.
unsafe fn service_of(binder: *mut AIBinder) -> &'static Service {
    let ptr = AIBinder_getUserData(binder);
    &*(ptr as *const Service)
}

/// The shipped service compared the binder calling uid against 1002 and only
/// answered below it (root 0 and system 1000 pass; vendor processes at 1002
/// and above are refused).
fn uid_allowed() -> bool {
    // SAFETY: read-only query of the calling-uid thread-local.
    let uid = unsafe { AIBinder_getCallingUid() };
    (uid as i32) < MAX_ALLOWED_UID_EXCLUSIVE
}


fn log_denied_uid() {
    log(
        LOG_LEVEL_ERROR,
        LOG_TAG,
        "The user id is not allowed, return!",
    );
}

fn i32_reply(out: *mut AParcel, value: i32) -> Result<(), BinderStatus> {
    write_ok_status(out)?;
    write_i32(out, value)
}

fn string_reply(out: *mut AParcel, value: String) -> Result<(), BinderStatus> {
    write_ok_status(out)?;
    write_string(out, &value)
}

fn write_state(out: *mut AParcel, state: &PowerStateSubsystemSleepState) -> Result<(), BinderStatus> {
    let start = begin_parcelable(out)?;
    write_string(out, &state.name)?;
    write_i64(out, state.version)?;
    write_i64(out, state.residency_in_msec_since_boot)?;
    write_i64(out, state.total_transitions)?;
    write_i64(out, state.last_entry_timestamp_ms)?;
    write_i32(out, if state.supported_only_in_suspend { 1 } else { 0 })?;
    end_parcelable(out, start)
}

fn write_get_single(
    out: *mut AParcel,
    state: &PowerStateSubsystemSleepState,
    result: i32,
) -> Result<(), BinderStatus> {
    write_ok_status(out)?;
    write_state(out, state)?;
    write_i32(out, result)
}

fn write_get_list(
    out: *mut AParcel,
    list: &[PowerStateSubsystemSleepState],
    result: i32,
) -> Result<(), BinderStatus> {
    write_ok_status(out)?;
    write_i32(out, list.len() as i32)?;
    for state in list {
        write_state(out, state)?;
    }
    write_i32(out, result)
}

fn write_trigger_dump(out: *mut AParcel, result: i32) -> Result<(), BinderStatus> {
    write_ok_status(out)?;
    write_i32(out, result)
}
