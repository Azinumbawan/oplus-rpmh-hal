// Subsystem sleep-state parcelable for the OPlus RPMH HAL (V1).
//
// Field order and types define the parcel layout, so they are fixed by the
// wire contract described alongside the interface.
package vendor.oplus.hardware.rpmh;

parcelable PowerStateSubsystemSleepState {
    String name;
    long version;
    long residencyInMsecSinceBoot;
    long totalTransitions;
    long lastEntryTimestampMs;
    boolean supportedOnlyInSuspend;
}
