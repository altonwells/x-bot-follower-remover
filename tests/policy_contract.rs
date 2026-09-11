use serde_json::Value;
use x_bot_follower_remover::model::{Account, Policy};
#[test]
fn policy_contract_matches_extension_fixtures() {
    let f: Value = serde_json::from_str(include_str!("../protocol/fixtures/policy.json")).unwrap();
    for c in f["cases"].as_array().unwrap() {
        let mut policy = f["policy"].clone();
        if let Some(overrides) = c["policy"].as_object() {
            for (key, value) in overrides {
                policy[key] = value.clone();
            }
        }
        let p: Policy = serde_json::from_value(policy).unwrap();
        let mut a = f["account"].clone();
        for (k, v) in c["changes"].as_object().unwrap() {
            a[k] = v.clone();
        }
        let a: Account = serde_json::from_value(a).unwrap();
        assert_eq!(
            a.reason(&p, f["now_ms"].as_i64().unwrap()).is_ok(),
            c["eligible"].as_bool().unwrap(),
            "{}",
            c["name"]
        );
    }
}
