use crate::model::{DiffItem, ProvenanceOrigin};
use serde_json::Value;

pub fn diff_json(
    expected_str: &str,
    actual_str: &str,
    origin: ProvenanceOrigin,
) -> Option<Vec<DiffItem>> {
    let expected_val: Value = serde_json::from_str(expected_str).ok()?;
    let actual_val: Value = serde_json::from_str(actual_str).ok()?;

    let mut diffs = Vec::new();
    compare_values("", &expected_val, &actual_val, origin, &mut diffs);
    Some(diffs)
}

fn compare_values(
    path: &str,
    expected: &Value,
    actual: &Value,
    origin: ProvenanceOrigin,
    diffs: &mut Vec<DiffItem>,
) {
    match (expected, actual) {
        (Value::Object(exp_map), Value::Object(act_map)) => {
            for (key, exp_v) in exp_map {
                let sub_path = format!("{}/{}", path, escape_json_pointer(key));
                if let Some(act_v) = act_map.get(key) {
                    compare_values(&sub_path, exp_v, act_v, origin, diffs);
                } else {
                    diffs.push(DiffItem {
                        path: sub_path,
                        diff_type: "Removed".to_string(),
                        expected: Some(exp_v.to_string()),
                        actual: None,
                        origin,
                    });
                }
            }

            for (key, act_v) in act_map {
                if !exp_map.contains_key(key) {
                    let sub_path = format!("{}/{}", path, escape_json_pointer(key));
                    diffs.push(DiffItem {
                        path: sub_path,
                        diff_type: "Added".to_string(),
                        expected: None,
                        actual: Some(act_v.to_string()),
                        origin,
                    });
                }
            }
        }
        (Value::Array(exp_arr), Value::Array(act_arr)) => {
            let max_len = std::cmp::max(exp_arr.len(), act_arr.len());
            for i in 0..max_len {
                let sub_path = format!("{}/{}", path, i);
                match (exp_arr.get(i), act_arr.get(i)) {
                    (Some(e), Some(a)) => {
                        compare_values(&sub_path, e, a, origin, diffs);
                    }
                    (Some(e), None) => {
                        diffs.push(DiffItem {
                            path: sub_path,
                            diff_type: "Removed".to_string(),
                            expected: Some(e.to_string()),
                            actual: None,
                            origin,
                        });
                    }
                    (None, Some(a)) => {
                        diffs.push(DiffItem {
                            path: sub_path,
                            diff_type: "Added".to_string(),
                            expected: None,
                            actual: Some(a.to_string()),
                            origin,
                        });
                    }
                    (None, None) => {}
                }
            }
        }
        (exp, act) if exp != act => {
            let diff_type = if discriminant_name(exp) != discriminant_name(act) {
                "TypeMismatch"
            } else {
                "Changed"
            };

            diffs.push(DiffItem {
                path: if path.is_empty() { "/" } else { path }.to_string(),
                diff_type: diff_type.to_string(),
                expected: Some(exp.to_string()),
                actual: Some(act.to_string()),
                origin,
            });
        }
        _ => {}
    }
}

fn discriminant_name(val: &Value) -> &'static str {
    match val {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn escape_json_pointer(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

pub fn diff_scalar(
    path: &str,
    expected: Option<&str>,
    actual: Option<&str>,
    origin: ProvenanceOrigin,
) -> Option<DiffItem> {
    if expected == actual {
        return None;
    }

    Some(DiffItem {
        path: path.to_string(),
        diff_type: "Changed".to_string(),
        expected: expected.map(String::from),
        actual: actual.map(String::from),
        origin,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_diff_added_and_removed() {
        let expected = r#"{"a": 1, "b": "hello"}"#;
        let actual = r#"{"b": "hello", "c": true}"#;

        let diffs = diff_json(expected, actual, ProvenanceOrigin::Derived).unwrap();
        assert_eq!(diffs.len(), 2);

        let removed = diffs.iter().find(|d| d.path == "/a").unwrap();
        assert_eq!(removed.diff_type, "Removed");

        let added = diffs.iter().find(|d| d.path == "/c").unwrap();
        assert_eq!(added.diff_type, "Added");
    }

    #[test]
    fn test_json_diff_type_mismatch() {
        let expected = r#"{"amount": 100}"#;
        let actual = r#"{"amount": "100"}"#;

        let diffs = diff_json(expected, actual, ProvenanceOrigin::Derived).unwrap();
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].diff_type, "TypeMismatch");
    }

    #[test]
    fn test_json_diff_key_order_independence() {
        let expected = r#"{"x": 1, "y": 2}"#;
        let actual = r#"{"y": 2, "x": 1}"#;

        let diffs = diff_json(expected, actual, ProvenanceOrigin::Derived).unwrap();
        assert_eq!(diffs.len(), 0);
    }
}
