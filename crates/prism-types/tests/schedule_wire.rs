//! Wire-format pin: ScheduleTrigger serializes exactly as the generated zod
//! schema expects (tagged, kebab-case variants, camelCase fields).

// Integration-test context: unwraps on invariant failures are the test's
// failure mode (workspace denies unwrap in src/, tests override here).
#![allow(clippy::unwrap_used)]

use prism_types::commands::ScheduleTrigger;

#[test]
fn schedule_trigger_wire_is_camel() {
    let t = ScheduleTrigger::Daily { time_min: 540 };
    let j = serde_json::to_string(&t).unwrap();
    assert!(j.contains("\"timeMin\""), "camelCase fields: {j}");
    let w = ScheduleTrigger::Weekly {
        days: vec![1, 5],
        time_min: 600,
    };
    let jw = serde_json::to_string(&w).unwrap();
    assert!(
        jw.contains("\"timeMin\"") && jw.contains("\"days\""),
        "{jw}"
    );
    let l = ScheduleTrigger::AtLogon {};
    assert_eq!(
        serde_json::to_string(&l).unwrap(),
        "{\"kind\":\"at-logon\"}"
    );
    // Round trip through the exact zod-validated shape.
    let parsed: ScheduleTrigger =
        serde_json::from_str("{\"kind\":\"daily\",\"timeMin\":540}").unwrap();
    assert_eq!(parsed, t);
}
