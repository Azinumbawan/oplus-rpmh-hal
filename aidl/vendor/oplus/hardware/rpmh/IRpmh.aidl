// AIDL for the OPlus RPMH HAL (V1).
//
// Reconstructed from the shipped artifacts:
//   erofs/vendor/bin/hw/vendor.oplus.hardware.rpmh-V1-service
//   erofs/vendor/lib64/vendor.oplus.hardware.rpmh-V1-ndk_platform.so
//
// Interface hash (embedded in both binaries as the AIDL interface hash):
//   acf5f9a3960fb9a2f76f86947dff5ca9e0c86e05
//
// Method order is load-bearing: AIDL assigns transaction codes by declaration
// order (FIRST_CALL_TRANSACTION == 1), and the shipped binary dispatches on
// exactly this order. Verified against the Java stub in
// system/system/framework/oplus-framework.jar (TRANSACTION_* constants).
package vendor.oplus.hardware.rpmh;

import vendor.oplus.hardware.rpmh.PowerStateSubsystemSleepState;
import vendor.oplus.hardware.rpmh.Result;

interface IRpmh {
    Result getPowerStateSubsystemSleepState(String name, out PowerStateSubsystemSleepState subsystem);
    Result getPowerStateSubsystemSleepStateList(out List<PowerStateSubsystemSleepState> subsystemList);
    int triggerDumpType(int type);
}
