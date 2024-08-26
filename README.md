This is still very much a work in progress.

# Building

1. checkout the repository
2. `git submodule update --init --recursive`
3. `cargo test`
4. `cargo run`
5. `cargo build`

# Organization

1. `crates/libfsm` - a proc_macro wrapper around the Rust bindings
2. `crates/libfsm_api` - rustbindings to the C API
3. `crates/libfsm_test` - a test program/example showing the proc_macro