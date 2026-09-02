//! THE HOST STUBS ARE WHAT LET A CONSUMER CHECK THEIR GUEST WITHOUT NAMING THE
//! WASM TARGET, so their message is a contract and this holds them to it.
//!
//! The pilot had to add a build tag to its make check because the Go half's
//! did not exist and the standard toolchain reported Emit undefined on its own
//! data guest. That this file COMPILES at all is half the point: it is a host
//! build naming all three methods.

use crate::plan::Lib;

const HOST_ONLY: &str = "fkrecipes: Emit runs only inside a wasm guest; this build is for the host";

#[test]
fn emit_panics_on_the_host() {
    let lib = Lib::new();
    let got = std::panic::catch_unwind(|| lib.emit()).expect_err("emit returned on the host");
    let msg = got
        .downcast_ref::<String>()
        .map(|s| s.as_str())
        .expect("the panic payload is not a string");
    assert_eq!(msg, HOST_ONLY);
}

#[test]
fn emit_settings_panics_on_the_host() {
    let lib = Lib::new();
    let got = std::panic::catch_unwind(|| lib.emit_settings()).expect_err("emit_settings returned");
    let msg = got
        .downcast_ref::<String>()
        .map(|s| s.as_str())
        .expect("the panic payload is not a string");
    assert_eq!(msg, HOST_ONLY);
}

#[test]
fn emit_data_panics_on_the_host() {
    let lib = Lib::new();
    let got = std::panic::catch_unwind(|| lib.emit_data()).expect_err("emit_data returned");
    let msg = got
        .downcast_ref::<String>()
        .map(|s| s.as_str())
        .expect("the panic payload is not a string");
    assert_eq!(msg, HOST_ONLY);
}
