// Result enum for the OPlus RPMH HAL.
//
// The backing type must be "int": the Result out-parameter is a 32-bit value on
// the wire, while aidl defaults to "byte", which would change the parcel width
// and corrupt the reply.
//
// The enum is a top-level type in package vendor.oplus.hardware.rpmh, next to
// IRpmh.

package vendor.oplus.hardware.rpmh;

@Backing(type="int")
@VintfStability
enum Result {
    UN_SUPPORTED = -1,
    PERMISSION_DENIED = -2,
    BAD_VALUE = -3,
    INVALID_OPERATION = -4,
    OK = 0,
}
