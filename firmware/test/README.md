# Firmware logic tests (host)

The board's brain — the work-state machine (`machine.cpp`) and the XP/growth
engine (`growth.cpp`) — is plain C++ with no hardware dependencies, so it is
compiled and tested on the host against the Arduino stubs in `stub/`.

    make -C firmware/test

The checks mirror the Rust unit tests in `pc/petd/src/state.rs` and
`growth.rs`, which is what keeps the two editions of DevPet behaving
identically.
