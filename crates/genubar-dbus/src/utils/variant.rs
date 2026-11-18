use std::collections::HashMap;
use zbus::{OwnedVariant, Variant};

pub fn unwrap_val<'a>(mut val: &'a Variant<'a>) -> &'a Variant<'a> {
    while let Variant::Variant(inner) = val {
        val = inner.as_ref();
    }
    val
}

pub fn get_str_prop(props: &HashMap<String, OwnedVariant>, key: &str) -> Option<String> {
    let raw: &Variant = props.get(key)?;
    match unwrap_val(raw) {
        Variant::Str(s) => Some(s.to_string()),
        _ => None,
    }
}

pub fn get_bool_prop(props: &HashMap<String, OwnedVariant>, key: &str, default: bool) -> bool {
    let Some(raw) = props.get(key) else {
        return default;
    };
    match unwrap_val(raw) {
        Variant::Bool(b) => *b,
        _ => default,
    }
}

pub fn get_i32_prop(props: &HashMap<String, OwnedVariant>, key: &str, default: i32) -> i32 {
    let Some(raw) = props.get(key) else {
        return default;
    };
    match unwrap_val(raw) {
        Variant::I32(n) => *n,
        _ => default,
    }
}