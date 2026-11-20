// The platform-independent half of the OPlus Rpmh HAL.
//
// The platform parsers, the subsystem-sleep-state adapters and the
// trigger-dump paths live here so they are unit-testable on the host. The
// binder-facing half (wire.rs / hal.rs) is an Android-only binary in
// src/bin/rpmh_service.rs.

pub mod core;
pub mod io_shim;
pub mod protocol;
