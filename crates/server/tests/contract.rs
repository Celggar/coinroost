//! The committed contract is the one the server generates.

/// Regenerate with `cargo run -p coinroost-server -- openapi > spec/openapi.json`.
#[test]
fn openapi_contract_is_committed() {
    let generated = coinroost_server::openapi().to_pretty_json().unwrap() + "\n";
    let committed = include_str!("../../../spec/openapi.json");
    assert!(
        generated == committed,
        "spec/openapi.json is out of date: run `cargo run -p coinroost-server -- openapi > spec/openapi.json`"
    );
}
