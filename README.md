# OplusRpmh

Pure-Rust implementation of the OPlus RPMH (resource power manager
hardened) AIDL HAL (`vendor.oplus.hardware.rpmh.IRpmh` V1) for the Infinix GT
10 Pro (X6739, MT6893).

Reconstructed from the shipped OPlus binaries and rewritten in Rust: no C or
C++ source, no prebuilt library. The binder transport is hand-rolled against
`libbinder_ndk`'s stable C API at the exact wire format the generated clients
use, and the interface hash is served verbatim. APSS sleep states are fed
from the MediaTek SPM counters that exist on this kernel, in the shape the
OPlus framework polls.

See [NOTES.md](NOTES.md) for the contract, the node-adaptation table and the
build instructions.
