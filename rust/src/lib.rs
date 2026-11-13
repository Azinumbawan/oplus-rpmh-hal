// The cross-platform half of the OPlus Rpmh HAL port.
//
// The platform parsers, the X6739 subsystem-sleep-state adapters and the
// trigger-dump paths live here so they are unit-testable on the host. The
// binder-facing half (wire.rs / hal.rs) is an Android-only binary in
// src/bin/rpmh_service.rs.

pub mod core;
pub mod io_shim;
