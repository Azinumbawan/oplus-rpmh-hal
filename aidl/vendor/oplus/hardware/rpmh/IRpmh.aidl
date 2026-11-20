// AIDL for the OPlus RPMH HAL (V1).
//
// Interface hash:
//   acf5f9a3960fb9a2f76f86947dff5ca9e0c86e05
//
// Method order is load-bearing: AIDL assigns transaction codes by declaration
// order (FIRST_CALL_TRANSACTION == 1), so the wire contract depends on the
// declaration order below.
package vendor.oplus.hardware.rpmh;

import vendor.oplus.hardware.rpmh.PowerStateSubsystemSleepState;
import vendor.oplus.hardware.rpmh.Result;

@VintfStability
interface IRpmh {
    Result getPowerStateSubsystemSleepState(String name, out PowerStateSubsystemSleepState subsystem);
    Result getPowerStateSubsystemSleepStateList(out List<PowerStateSubsystemSleepState> subsystemList);
    int triggerDumpType(int type);
}
