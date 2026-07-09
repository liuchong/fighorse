//! Golden-output parity test: the Rust coverage report must match the
//! `figma-api coverage --format json` output byte-for-byte
//! (modulo trailing newline), including key ordering.

use fighorse::api::coverage;

#[test]
fn coverage_report_matches_golden() {
    let golden = include_str!("fixtures/coverage.json");
    let report = coverage::coverage_report();
    let rendered = serde_json::to_string_pretty(&report).unwrap();
    assert_eq!(rendered.trim_end(), golden.trim_end());
}
