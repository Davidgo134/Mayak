//! MessagePack -> JSON, for logs. lossy: Binary/Ext turn into base64 strings,
//! non-string map keys get stringified (JSON holds neither) — so it reads but
//! won't round-trip back to the same msgpack.

use base64::Engine;
use rmpv::Value;
use serde_json::{Map, Number, Value as Json};

/// a decoded MessagePack value as JSON. `Binary` -> plain base64 string
/// (readable, but a request can't recover it — see [`value_to_json_tagged`]).
pub fn value_to_json(value: &Value) -> Json {
    to_json(value, false)
}

/// like [`value_to_json`], but `Binary` -> `{"$bin":"<base64>"}`, which
/// [`json_to_value`] turns back — round-trips on the data plane.
pub fn value_to_json_tagged(value: &Value) -> Json {
    to_json(value, true)
}

fn to_json(value: &Value, tag_binary: bool) -> Json {
    match value {
        Value::Nil => Json::Null,
        Value::Boolean(b) => Json::Bool(*b),
        Value::Integer(i) => integer_to_json(i),
        Value::F32(f) => Number::from_f64(*f as f64).map_or(Json::Null, Json::Number),
        Value::F64(f) => Number::from_f64(*f).map_or(Json::Null, Json::Number),
        Value::String(s) => Json::String(utf8_lossy(s)),
        Value::Binary(bytes) => {
            if tag_binary {
                let mut map = Map::with_capacity(1);
                map.insert("$bin".to_string(), Json::String(base64_encode(bytes)));
                Json::Object(map)
            } else {
                Json::String(base64_encode(bytes))
            }
        }
        Value::Array(items) => Json::Array(items.iter().map(|v| to_json(v, tag_binary)).collect()),
        Value::Map(entries) => {
            let mut map = Map::with_capacity(entries.len());
            for (k, v) in entries {
                map.insert(map_key(k), to_json(v, tag_binary));
            }
            Json::Object(map)
        }
        Value::Ext(tag, data) => {
            let mut map = Map::with_capacity(2);
            map.insert("$ext".to_string(), Json::Number((*tag).into()));
            map.insert("data".to_string(), Json::String(base64_encode(data)));
            Json::Object(map)
        }
    }
}

/// JSON to MessagePack, to build a request from a host map. Inverse of
/// [`value_to_json`] where it can be: `{"$bin":"<b64>"}` -> `Binary`,
/// `{"$ext": tag, "data": "<b64>"}` -> `Ext`; other keys stay text (JSON has no
/// non-string keys).
pub fn json_to_value(value: &Json) -> Value {
    match value {
        Json::Null => Value::Nil,
        Json::Bool(b) => Value::from(*b),
        Json::Number(n) => number_to_value(n),
        Json::String(s) => Value::from(s.clone()),
        Json::Array(items) => Value::Array(items.iter().map(json_to_value).collect()),
        Json::Object(obj) => {
            if let Some(v) = tagged_binary(obj) {
                return v;
            }
            Value::Map(
                obj.iter()
                    .map(|(k, v)| (Value::from(k.clone()), json_to_value(v)))
                    .collect(),
            )
        }
    }
}

fn number_to_value(n: &Number) -> Value {
    if let Some(i) = n.as_i64() {
        Value::from(i)
    } else if let Some(u) = n.as_u64() {
        Value::from(u)
    } else if let Some(f) = n.as_f64() {
        Value::from(f)
    } else {
        Value::Nil
    }
}

fn tagged_binary(obj: &Map<String, Json>) -> Option<Value> {
    if obj.len() == 1 {
        if let Some(Json::String(b64)) = obj.get("$bin") {
            return base64_decode(b64).map(Value::Binary);
        }
    }
    if obj.len() == 2 {
        if let (Some(Json::Number(tag)), Some(Json::String(b64))) =
            (obj.get("$ext"), obj.get("data"))
        {
            if let (Some(t), Some(bytes)) = (tag.as_i64(), base64_decode(b64)) {
                return Some(Value::Ext(t as i8, bytes));
            }
        }
    }
    None
}

fn base64_decode(s: &str) -> Option<Vec<u8>> {
    base64::engine::general_purpose::STANDARD.decode(s).ok()
}

fn integer_to_json(i: &rmpv::Integer) -> Json {
    if let Some(u) = i.as_u64() {
        Json::Number(u.into())
    } else if let Some(s) = i.as_i64() {
        Json::Number(s.into())
    } else {
        Json::Null
    }
}

fn map_key(value: &Value) -> String {
    match value {
        Value::String(s) => utf8_lossy(s),
        Value::Integer(i) => integer_to_json(i).to_string(),
        Value::Boolean(b) => b.to_string(),
        Value::Nil => "null".to_string(),
        other => value_to_json(other).to_string(),
    }
}

fn utf8_lossy(s: &rmpv::Utf8String) -> String {
    match s.as_str() {
        Some(text) => text.to_string(),
        None => String::from_utf8_lossy(s.as_bytes()).into_owned(),
    }
}

fn base64_encode(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::value_to_json;
    use rmpv::Value;

    #[test]
    fn renders_scalars_and_nested_maps() {
        let value = Value::Map(vec![
            (Value::from("id"), Value::from(42u64)),
            (Value::from("ok"), Value::from(true)),
            (
                Value::from("tags"),
                Value::Array(vec![Value::from("a"), Value::from("b")]),
            ),
        ]);
        let json = value_to_json(&value);
        assert_eq!(json["id"], 42);
        assert_eq!(json["ok"], true);
        assert_eq!(json["tags"][1], "b");
    }

    #[test]
    fn binary_becomes_base64_and_int_keys_stringify() {
        let value = Value::Map(vec![(Value::from(7), Value::Binary(vec![0xDE, 0xAD]))]);
        let json = value_to_json(&value);
        assert_eq!(json["7"], "3q0=");
    }

    #[test]
    fn tagged_binary_round_trips_flat_stays_base64() {
        use super::{json_to_value, value_to_json_tagged};
        let value = Value::Map(vec![
            (
                Value::from("fp"),
                Value::Binary(vec![0xDE, 0xAD, 0xBE, 0xEF]),
            ),
            (Value::from("name"), Value::from("x")),
        ]);
        // tagged output round-trips back to the same msgpack Binary
        let back = json_to_value(&value_to_json_tagged(&value));
        assert_eq!(back, value);
        // flat output keeps binary as a plain base64 string (for logs)
        assert_eq!(value_to_json(&value)["fp"], "3q2+7w==");
        assert!(value_to_json_tagged(&value)["fp"].is_object());
    }

    #[test]
    fn json_to_value_builds_map_and_recovers_binary() {
        use super::json_to_value;
        let json = serde_json::json!({
            "id": 42,
            "ok": true,
            "name": "x",
            "fp": { "$bin": "3q0=" },
            "list": [1, 2, 3],
        });
        let value = json_to_value(&json);
        let map = value.as_map().unwrap();
        let get = |k: &str| {
            map.iter()
                .find(|(mk, _)| mk.as_str() == Some(k))
                .map(|(_, v)| v)
        };
        assert_eq!(get("id").unwrap().as_i64(), Some(42));
        assert_eq!(get("ok").unwrap().as_bool(), Some(true));
        assert_eq!(get("fp").unwrap().as_slice(), Some(&[0xDE, 0xAD][..]));
        assert_eq!(get("list").unwrap().as_array().unwrap().len(), 3);
    }
}
