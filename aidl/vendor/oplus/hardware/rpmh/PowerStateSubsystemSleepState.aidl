// Parcelable for the OPlus RPMH HAL (V1).
//
// Reconstructed from the AIDL Java stub
// system/system/framework/oplus-framework.jar:
//   vendor.oplus.hardware.rpmh.PowerStateSubsystemSleepState
//
// Field order and types are taken verbatim from the generated
// readFromParcel()/writeToParcel() in that stub, so the parcel layout matches
// the shipped HAL byte for byte.
package vendor.oplus.hardware.rpmh;

parcelable PowerStateSubsystemSleepState {
    String name;
    long version;
    long residencyInMsecSinceBoot;
    long totalTransitions;
    long lastEntryTimestampMs;
    boolean supportedOnlyInSuspend;
}
