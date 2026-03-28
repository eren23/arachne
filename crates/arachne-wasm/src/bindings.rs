//! Low-level wasm-bindgen bindings and type conversion helpers.
//!
//! Provides conversions between JavaScript values and Arachne Rust types.
//! On native targets, uses stub wrappers so the conversion API is testable
//! without a WASM runtime.

use arachne_math::{Vec2, Vec3, Color};
use arachne_input::KeyCode;

// ---------------------------------------------------------------------------
// JsValue wrapper (native stub)
// ---------------------------------------------------------------------------

/// A wrapper around a JavaScript value.
///
/// On WASM with `wasm-bindgen`, this wraps `wasm_bindgen::JsValue`.
/// On native, it provides a simplified stub for testing type conversions.
#[derive(Clone, Debug)]
pub enum JsValueWrapper {
    Null,
    Undefined,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<JsValueWrapper>),
    Object(Vec<(String, JsValueWrapper)>),
}

impl JsValueWrapper {
    pub fn null() -> Self {
        JsValueWrapper::Null
    }

    pub fn undefined() -> Self {
        JsValueWrapper::Undefined
    }

    pub fn from_bool(v: bool) -> Self {
        JsValueWrapper::Bool(v)
    }

    pub fn from_f64(v: f64) -> Self {
        JsValueWrapper::Number(v)
    }

    pub fn from_str(v: impl Into<String>) -> Self {
        JsValueWrapper::String(v.into())
    }

    pub fn from_array(items: Vec<JsValueWrapper>) -> Self {
        JsValueWrapper::Array(items)
    }

    pub fn from_entries(entries: Vec<(String, JsValueWrapper)>) -> Self {
        JsValueWrapper::Object(entries)
    }

    pub fn is_null(&self) -> bool {
        matches!(self, JsValueWrapper::Null)
    }

    pub fn is_undefined(&self) -> bool {
        matches!(self, JsValueWrapper::Undefined)
    }

    pub fn is_nullish(&self) -> bool {
        self.is_null() || self.is_undefined()
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            JsValueWrapper::Bool(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            JsValueWrapper::Number(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_string(&self) -> Option<&str> {
        match self {
            JsValueWrapper::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[JsValueWrapper]> {
        match self {
            JsValueWrapper::Array(arr) => Some(arr.as_slice()),
            _ => None,
        }
    }

    pub fn get(&self, key: &str) -> Option<&JsValueWrapper> {
        match self {
            JsValueWrapper::Object(entries) => {
                entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
            }
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Type converter
// ---------------------------------------------------------------------------

/// Handles conversions between JS values and Arachne Rust types.
pub struct TypeConverter;

impl TypeConverter {
    pub fn new() -> Self {
        Self
    }

    // --- JS -> Rust conversions ---

    /// Convert a JS array [x, y] to a Vec2.
    pub fn js_to_vec2(&self, value: &JsValueWrapper) -> Option<Vec2> {
        let arr = value.as_array()?;
        if arr.len() < 2 {
            return None;
        }
        let x = arr[0].as_f64()? as f32;
        let y = arr[1].as_f64()? as f32;
        Some(Vec2::new(x, y))
    }

    /// Convert a JS array [x, y, z] to a Vec3.
    pub fn js_to_vec3(&self, value: &JsValueWrapper) -> Option<Vec3> {
        let arr = value.as_array()?;
        if arr.len() < 3 {
            return None;
        }
        let x = arr[0].as_f64()? as f32;
        let y = arr[1].as_f64()? as f32;
        let z = arr[2].as_f64()? as f32;
        Some(Vec3::new(x, y, z))
    }

    /// Convert a JS array [r, g, b, a] (0-255) to a Color.
    pub fn js_to_color(&self, value: &JsValueWrapper) -> Option<Color> {
        let arr = value.as_array()?;
        if arr.len() < 4 {
            return None;
        }
        let r = arr[0].as_f64()? as f32 / 255.0;
        let g = arr[1].as_f64()? as f32 / 255.0;
        let b = arr[2].as_f64()? as f32 / 255.0;
        let a = arr[3].as_f64()? as f32 / 255.0;
        Some(Color::new(r, g, b, a))
    }

    /// Convert a JS string key code to an Arachne KeyCode.
    pub fn js_to_key_code(&self, value: &JsValueWrapper) -> Option<KeyCode> {
        let s = value.as_string()?;
        crate::events::translate_key_code(s)
    }

    // --- Rust -> JS conversions ---

    /// Convert a Vec2 to a JS array [x, y].
    pub fn vec2_to_js(&self, v: Vec2) -> JsValueWrapper {
        JsValueWrapper::Array(vec![
            JsValueWrapper::Number(v.x as f64),
            JsValueWrapper::Number(v.y as f64),
        ])
    }

    /// Convert a Vec3 to a JS array [x, y, z].
    pub fn vec3_to_js(&self, v: Vec3) -> JsValueWrapper {
        JsValueWrapper::Array(vec![
            JsValueWrapper::Number(v.x as f64),
            JsValueWrapper::Number(v.y as f64),
            JsValueWrapper::Number(v.z as f64),
        ])
    }

    /// Convert a Color to a JS array [r, g, b, a] (0-255).
    pub fn color_to_js(&self, c: Color) -> JsValueWrapper {
        JsValueWrapper::Array(vec![
            JsValueWrapper::Number((c.r * 255.0) as f64),
            JsValueWrapper::Number((c.g * 255.0) as f64),
            JsValueWrapper::Number((c.b * 255.0) as f64),
            JsValueWrapper::Number((c.a * 255.0) as f64),
        ])
    }

    /// Convert an entity ID (u64) to a JS number.
    pub fn entity_to_js(&self, entity_id: u64) -> JsValueWrapper {
        JsValueWrapper::Number(entity_id as f64)
    }

    /// Convert a JS number to an entity ID.
    pub fn js_to_entity(&self, value: &JsValueWrapper) -> Option<u64> {
        value.as_f64().map(|v| v as u64)
    }

    /// Convert a boolean to a JsValueWrapper.
    pub fn bool_to_js(&self, v: bool) -> JsValueWrapper {
        JsValueWrapper::Bool(v)
    }

    /// Convert a string to a JsValueWrapper.
    pub fn string_to_js(&self, s: impl Into<String>) -> JsValueWrapper {
        JsValueWrapper::String(s.into())
    }
}

impl Default for TypeConverter {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn js_value_null_and_undefined() {
        let null = JsValueWrapper::null();
        assert!(null.is_null());
        assert!(null.is_nullish());

        let undef = JsValueWrapper::undefined();
        assert!(undef.is_undefined());
        assert!(undef.is_nullish());
    }

    #[test]
    fn js_value_bool() {
        let val = JsValueWrapper::from_bool(true);
        assert_eq!(val.as_bool(), Some(true));
        assert_eq!(val.as_f64(), None);
    }

    #[test]
    fn js_value_number() {
        let val = JsValueWrapper::from_f64(42.5);
        assert_eq!(val.as_f64(), Some(42.5));
        assert_eq!(val.as_bool(), None);
    }

    #[test]
    fn js_value_string() {
        let val = JsValueWrapper::from_str("hello");
        assert_eq!(val.as_string(), Some("hello"));
    }

    #[test]
    fn js_value_array() {
        let arr = JsValueWrapper::from_array(vec![
            JsValueWrapper::from_f64(1.0),
            JsValueWrapper::from_f64(2.0),
        ]);
        let items = arr.as_array().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].as_f64(), Some(1.0));
        assert_eq!(items[1].as_f64(), Some(2.0));
    }

    #[test]
    fn js_value_object() {
        let obj = JsValueWrapper::from_entries(vec![
            ("name".to_string(), JsValueWrapper::from_str("test")),
            ("value".to_string(), JsValueWrapper::from_f64(42.0)),
        ]);

        assert_eq!(obj.get("name").unwrap().as_string(), Some("test"));
        assert_eq!(obj.get("value").unwrap().as_f64(), Some(42.0));
        assert!(obj.get("missing").is_none());
    }

    // --- TypeConverter tests ---

    #[test]
    fn convert_js_to_vec2() {
        let tc = TypeConverter::new();
        let arr = JsValueWrapper::from_array(vec![
            JsValueWrapper::from_f64(10.0),
            JsValueWrapper::from_f64(20.0),
        ]);
        let v = tc.js_to_vec2(&arr).unwrap();
        assert!((v.x - 10.0).abs() < 1e-6);
        assert!((v.y - 20.0).abs() < 1e-6);
    }

    #[test]
    fn convert_js_to_vec2_too_short() {
        let tc = TypeConverter::new();
        let arr = JsValueWrapper::from_array(vec![JsValueWrapper::from_f64(1.0)]);
        assert!(tc.js_to_vec2(&arr).is_none());
    }

    #[test]
    fn convert_js_to_vec2_not_array() {
        let tc = TypeConverter::new();
        let val = JsValueWrapper::from_f64(42.0);
        assert!(tc.js_to_vec2(&val).is_none());
    }

    #[test]
    fn convert_js_to_vec3() {
        let tc = TypeConverter::new();
        let arr = JsValueWrapper::from_array(vec![
            JsValueWrapper::from_f64(1.0),
            JsValueWrapper::from_f64(2.0),
            JsValueWrapper::from_f64(3.0),
        ]);
        let v = tc.js_to_vec3(&arr).unwrap();
        assert!((v.x - 1.0).abs() < 1e-6);
        assert!((v.y - 2.0).abs() < 1e-6);
        assert!((v.z - 3.0).abs() < 1e-6);
    }

    #[test]
    fn convert_js_to_color() {
        let tc = TypeConverter::new();
        let arr = JsValueWrapper::from_array(vec![
            JsValueWrapper::from_f64(255.0),
            JsValueWrapper::from_f64(128.0),
            JsValueWrapper::from_f64(0.0),
            JsValueWrapper::from_f64(255.0),
        ]);
        let c = tc.js_to_color(&arr).unwrap();
        assert!((c.r - 1.0).abs() < 0.01);
        assert!((c.g - 0.502).abs() < 0.01);
        assert!((c.b - 0.0).abs() < 0.01);
        assert!((c.a - 1.0).abs() < 0.01);
    }

    #[test]
    fn convert_js_to_key_code() {
        let tc = TypeConverter::new();
        let val = JsValueWrapper::from_str("KeyW");
        assert_eq!(tc.js_to_key_code(&val), Some(KeyCode::W));

        let val = JsValueWrapper::from_str("Space");
        assert_eq!(tc.js_to_key_code(&val), Some(KeyCode::Space));

        let val = JsValueWrapper::from_str("Unknown");
        assert_eq!(tc.js_to_key_code(&val), None);
    }

    #[test]
    fn convert_vec2_to_js() {
        let tc = TypeConverter::new();
        let js = tc.vec2_to_js(Vec2::new(3.14, 2.71));
        let arr = js.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert!((arr[0].as_f64().unwrap() - 3.14).abs() < 0.001);
        assert!((arr[1].as_f64().unwrap() - 2.71).abs() < 0.001);
    }

    #[test]
    fn convert_vec3_to_js() {
        let tc = TypeConverter::new();
        let js = tc.vec3_to_js(Vec3::new(1.0, 2.0, 3.0));
        let arr = js.as_array().unwrap();
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[0].as_f64(), Some(1.0));
        assert_eq!(arr[1].as_f64(), Some(2.0));
        assert_eq!(arr[2].as_f64(), Some(3.0));
    }

    #[test]
    fn convert_color_to_js() {
        let tc = TypeConverter::new();
        let js = tc.color_to_js(Color::new(1.0, 0.5, 0.0, 1.0));
        let arr = js.as_array().unwrap();
        assert_eq!(arr.len(), 4);
        assert!((arr[0].as_f64().unwrap() - 255.0).abs() < 0.5);
        assert!((arr[1].as_f64().unwrap() - 127.5).abs() < 0.5);
        assert!((arr[2].as_f64().unwrap() - 0.0).abs() < 0.5);
        assert!((arr[3].as_f64().unwrap() - 255.0).abs() < 0.5);
    }

    #[test]
    fn convert_entity_round_trip() {
        let tc = TypeConverter::new();
        let entity_id: u64 = 12345;
        let js = tc.entity_to_js(entity_id);
        let back = tc.js_to_entity(&js).unwrap();
        assert_eq!(back, entity_id);
    }

    #[test]
    fn convert_bool_and_string() {
        let tc = TypeConverter::new();
        let b = tc.bool_to_js(true);
        assert_eq!(b.as_bool(), Some(true));

        let s = tc.string_to_js("hello");
        assert_eq!(s.as_string(), Some("hello"));
    }

    #[test]
    fn js_value_not_nullish() {
        let val = JsValueWrapper::from_f64(0.0);
        assert!(!val.is_nullish());

        let val = JsValueWrapper::from_str("");
        assert!(!val.is_nullish());

        let val = JsValueWrapper::from_bool(false);
        assert!(!val.is_nullish());
    }

    #[test]
    fn convert_vec3_too_short_returns_none() {
        let tc = TypeConverter::new();
        let arr = JsValueWrapper::from_array(vec![
            JsValueWrapper::from_f64(1.0),
            JsValueWrapper::from_f64(2.0),
        ]);
        assert!(tc.js_to_vec3(&arr).is_none());
    }

    #[test]
    fn convert_color_too_short_returns_none() {
        let tc = TypeConverter::new();
        let arr = JsValueWrapper::from_array(vec![
            JsValueWrapper::from_f64(255.0),
            JsValueWrapper::from_f64(128.0),
        ]);
        assert!(tc.js_to_color(&arr).is_none());
    }
}
