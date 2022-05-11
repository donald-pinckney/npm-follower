use serde_json::Value;

pub fn unwrap_array(v: Value) -> Vec<Value> {
    match v {
        Value::Array(a) => a,
        _ => panic!()
    }
}

pub fn unwrap_string(v: Value) -> Result<String, String> {
    match v {
        Value::String(s) => Ok(s),
        _ => Err(format!("Expected string, got: {:?}", v))
    }
}

pub fn unwrap_object(v: Value) -> Result<serde_json::Map<String, Value>, String> {
    match v {
        Value::Object(o) => Ok(o),
        _ => Err(format!("Expected object, got: {:?}", v))
    }
}

pub fn unwrap_number(v: Value) -> serde_json::Number {
    match v {
        Value::Number(n) => n,
        _ => panic!()
    }
}