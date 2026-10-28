//! Every fixture in `spec/fixtures/<profile>/` must be reproduced exactly: same transactions (amount text,
//! minor units, description, `external_id`), same error rows, same counts.

use std::path::{Path, PathBuf};

use coinroost_normalizer::{Profile, normalize};
use serde_json::Value;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec/fixtures")
}

#[test]
fn every_fixture_matches_its_expected_output() {
    let mut checked = 0;
    for entry in std::fs::read_dir(fixtures()).expect("spec/fixtures exists") {
        let dir = entry.expect("readable entry").path();
        let expected_path = dir.join("expected.json");
        if !expected_path.exists() {
            continue;
        }
        let expected: Value = serde_json::from_slice(&std::fs::read(&expected_path).unwrap()).unwrap();
        let field = |name: &str| {
            expected[name]
                .as_str()
                .unwrap_or_else(|| panic!("{name} in {expected_path:?}"))
        };
        let profile =
            Profile::find(field("profile")).unwrap_or_else(|| panic!("profile {}", field("profile")));
        let input = std::fs::read(dir.join(field("input"))).unwrap();
        let actual = match normalize(&input, &profile, field("namespace")) {
            Ok(result) => result.to_json(),
            Err(error) => panic!("{dir:?}: {error}"),
        };
        let mut wanted = expected["result"].clone();
        let mut got = actual;
        // The reference reports error rows by number; the problem text is this library's own.
        for value in [&mut wanted, &mut got] {
            for error in value["errors"].as_array_mut().into_iter().flatten() {
                error.as_object_mut().unwrap().retain(|key, _| key == "row");
            }
        }
        assert_eq!(got, wanted, "fixture {dir:?}");
        checked += 1;
    }
    assert!(
        checked >= 3,
        "expected at least three fixtures, checked {checked}"
    );
}
