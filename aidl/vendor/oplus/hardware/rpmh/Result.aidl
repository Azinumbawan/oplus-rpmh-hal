// AIDL for the OPlus RPMH (Result) enum.
//
// Reconstructed from the shipped artifacts:
//   erofs/vendor/lib64/vendor.oplus.hardware.rpmh-V1-ndk_platform.so
//   erofs/system/system/framework/services.jar (vendor.oplus.hardware.rpmh.Result)
//
// The shipped sidecar serialises this enum with AParcel_writeInt32 and reads
// it back with a 32-bit load (verified by disassembly of its onTransact), so
// the backing type must be "int". aidl defaults to "byte", which would change
// the parcel width and corrupt the out-parameter.
//
// The enum is a top-level type in package vendor.oplus.hardware.rpmh: the
// framework's Java stub exposes it as vendor.oplus.hardware.rpmh.Result, a
// separate class next to IRpmh, not a nested type.

package vendor.oplus.hardware.rpmh;

@Backing(type="int")
enum Result {
    UN_SUPPORTED = -1,
    PERMISSION_DENIED = -2,
    BAD_VALUE = -3,
    INVALID_OPERATION = -4,
    OK = 0,
}
