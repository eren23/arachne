/// Scene definition with hand-rolled JSON parser/writer.
/// No serde_json dependency — kept minimal for WASM size.

// ─── JSON Value ───────────────────────────────────────────────────────────────

/// A lightweight JSON value. Objects use `Vec` to preserve insertion order.
#[derive(Clone, Debug, PartialEq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Number(f64),
    Str(String),
    Array(Vec<JsonValue>),
    Object(Vec<(String, JsonValue)>),
}

impl JsonValue {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            JsonValue::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            JsonValue::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            JsonValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[JsonValue]> {
        match self {
            JsonValue::Array(a) => Some(a),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&[(String, JsonValue)]> {
        match self {
            JsonValue::Object(o) => Some(o),
            _ => None,
        }
    }

    /// Get a field from an object by key.
    pub fn get(&self, key: &str) -> Option<&JsonValue> {
        match self {
            JsonValue::Object(fields) => fields.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
}

// ─── JSON Parser ──────────────────────────────────────────────────────────────

struct JsonParser<'a> {
    input: &'a [u8],
    pos: usize,
}

impl<'a> JsonParser<'a> {
    fn new(input: &'a str) -> Self {
        JsonParser {
            input: input.as_bytes(),
            pos: 0,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.input.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<u8> {
        let ch = self.input.get(self.pos).copied();
        if ch.is_some() {
            self.pos += 1;
        }
        ch
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.input.len() {
            match self.input[self.pos] {
                b' ' | b'\t' | b'\n' | b'\r' => self.pos += 1,
                _ => break,
            }
        }
    }

    fn expect(&mut self, ch: u8) -> Result<(), String> {
        self.skip_whitespace();
        match self.advance() {
            Some(c) if c == ch => Ok(()),
            Some(c) => Err(format!(
                "expected '{}' at pos {}, found '{}'",
                ch as char, self.pos - 1, c as char
            )),
            None => Err(format!("expected '{}' at pos {}, found EOF", ch as char, self.pos)),
        }
    }

    fn parse_value(&mut self) -> Result<JsonValue, String> {
        self.skip_whitespace();
        match self.peek() {
            Some(b'"') => self.parse_string().map(JsonValue::Str),
            Some(b'{') => self.parse_object(),
            Some(b'[') => self.parse_array(),
            Some(b't') => self.parse_literal("true", JsonValue::Bool(true)),
            Some(b'f') => self.parse_literal("false", JsonValue::Bool(false)),
            Some(b'n') => self.parse_literal("null", JsonValue::Null),
            Some(c) if c == b'-' || c.is_ascii_digit() => self.parse_number(),
            Some(c) => Err(format!("unexpected char '{}' at pos {}", c as char, self.pos)),
            None => Err(format!("unexpected EOF at pos {}", self.pos)),
        }
    }

    fn parse_string(&mut self) -> Result<String, String> {
        self.expect(b'"')?;
        let mut s = String::new();
        loop {
            match self.advance() {
                Some(b'"') => return Ok(s),
                Some(b'\\') => {
                    match self.advance() {
                        Some(b'"') => s.push('"'),
                        Some(b'\\') => s.push('\\'),
                        Some(b'/') => s.push('/'),
                        Some(b'n') => s.push('\n'),
                        Some(b'r') => s.push('\r'),
                        Some(b't') => s.push('\t'),
                        Some(b'b') => s.push('\u{0008}'),
                        Some(b'f') => s.push('\u{000C}'),
                        Some(b'u') => {
                            let hex = self.take_n(4)?;
                            let code = u32::from_str_radix(&hex, 16)
                                .map_err(|_| format!("invalid unicode escape: \\u{}", hex))?;
                            let ch = char::from_u32(code)
                                .ok_or_else(|| format!("invalid unicode codepoint: {}", code))?;
                            s.push(ch);
                        }
                        Some(c) => return Err(format!("invalid escape: \\{}", c as char)),
                        None => return Err("unexpected EOF in string escape".into()),
                    }
                }
                Some(c) => s.push(c as char),
                None => return Err("unterminated string".into()),
            }
        }
    }

    fn take_n(&mut self, n: usize) -> Result<String, String> {
        if self.pos + n > self.input.len() {
            return Err("unexpected EOF".into());
        }
        let s = std::str::from_utf8(&self.input[self.pos..self.pos + n])
            .map_err(|e| format!("invalid UTF-8: {}", e))?
            .to_string();
        self.pos += n;
        Ok(s)
    }

    fn parse_number(&mut self) -> Result<JsonValue, String> {
        let start = self.pos;
        // Optional minus.
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        // Integer part.
        self.consume_digits();
        // Fraction.
        if self.peek() == Some(b'.') {
            self.pos += 1;
            self.consume_digits();
        }
        // Exponent.
        if matches!(self.peek(), Some(b'e') | Some(b'E')) {
            self.pos += 1;
            if matches!(self.peek(), Some(b'+') | Some(b'-')) {
                self.pos += 1;
            }
            self.consume_digits();
        }
        let num_str = std::str::from_utf8(&self.input[start..self.pos])
            .map_err(|e| format!("invalid UTF-8 in number: {}", e))?;
        let n: f64 = num_str
            .parse()
            .map_err(|_| format!("invalid number: {}", num_str))?;
        Ok(JsonValue::Number(n))
    }

    fn consume_digits(&mut self) {
        while self.pos < self.input.len() && self.input[self.pos].is_ascii_digit() {
            self.pos += 1;
        }
    }

    fn parse_object(&mut self) -> Result<JsonValue, String> {
        self.expect(b'{')?;
        self.skip_whitespace();
        let mut fields = Vec::new();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(JsonValue::Object(fields));
        }
        loop {
            self.skip_whitespace();
            let key = self.parse_string()?;
            self.expect(b':')?;
            let value = self.parse_value()?;
            fields.push((key, value));
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                }
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(JsonValue::Object(fields));
                }
                _ => return Err(format!("expected ',' or '}}' at pos {}", self.pos)),
            }
        }
    }

    fn parse_array(&mut self) -> Result<JsonValue, String> {
        self.expect(b'[')?;
        self.skip_whitespace();
        let mut items = Vec::new();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(JsonValue::Array(items));
        }
        loop {
            let value = self.parse_value()?;
            items.push(value);
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                }
                Some(b']') => {
                    self.pos += 1;
                    return Ok(JsonValue::Array(items));
                }
                _ => return Err(format!("expected ',' or ']' at pos {}", self.pos)),
            }
        }
    }

    fn parse_literal(&mut self, expected: &str, value: JsonValue) -> Result<JsonValue, String> {
        for &byte in expected.as_bytes() {
            match self.advance() {
                Some(c) if c == byte => {}
                _ => return Err(format!("expected literal '{}' at pos {}", expected, self.pos)),
            }
        }
        Ok(value)
    }
}

/// Parse a JSON string into a JsonValue.
pub fn parse_json(input: &str) -> Result<JsonValue, String> {
    let mut parser = JsonParser::new(input);
    let value = parser.parse_value()?;
    parser.skip_whitespace();
    if parser.pos != parser.input.len() {
        return Err(format!("trailing data at pos {}", parser.pos));
    }
    Ok(value)
}

// ─── JSON Writer ──────────────────────────────────────────────────────────────

/// Serialize a JsonValue into a compact JSON string.
pub fn write_json(value: &JsonValue) -> String {
    let mut out = String::new();
    write_value(&mut out, value);
    out
}

/// Serialize a JsonValue into a pretty-printed JSON string.
pub fn write_json_pretty(value: &JsonValue) -> String {
    let mut out = String::new();
    write_value_pretty(&mut out, value, 0);
    out
}

fn write_value(out: &mut String, value: &JsonValue) {
    match value {
        JsonValue::Null => out.push_str("null"),
        JsonValue::Bool(true) => out.push_str("true"),
        JsonValue::Bool(false) => out.push_str("false"),
        JsonValue::Number(n) => {
            if n.fract() == 0.0 && n.abs() < 1e15 {
                // Write as integer if it's a whole number.
                out.push_str(&format!("{}", *n as i64));
            } else {
                out.push_str(&format!("{}", n));
            }
        }
        JsonValue::Str(s) => write_json_string(out, s),
        JsonValue::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_value(out, item);
            }
            out.push(']');
        }
        JsonValue::Object(fields) => {
            out.push('{');
            for (i, (key, val)) in fields.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_json_string(out, key);
                out.push(':');
                write_value(out, val);
            }
            out.push('}');
        }
    }
}

fn write_value_pretty(out: &mut String, value: &JsonValue, indent: usize) {
    let indent_str = "  ";
    match value {
        JsonValue::Null => out.push_str("null"),
        JsonValue::Bool(true) => out.push_str("true"),
        JsonValue::Bool(false) => out.push_str("false"),
        JsonValue::Number(n) => {
            if n.fract() == 0.0 && n.abs() < 1e15 {
                out.push_str(&format!("{}", *n as i64));
            } else {
                out.push_str(&format!("{}", n));
            }
        }
        JsonValue::Str(s) => write_json_string(out, s),
        JsonValue::Array(items) => {
            if items.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push_str("[\n");
            for (i, item) in items.iter().enumerate() {
                for _ in 0..indent + 1 {
                    out.push_str(indent_str);
                }
                write_value_pretty(out, item, indent + 1);
                if i < items.len() - 1 {
                    out.push(',');
                }
                out.push('\n');
            }
            for _ in 0..indent {
                out.push_str(indent_str);
            }
            out.push(']');
        }
        JsonValue::Object(fields) => {
            if fields.is_empty() {
                out.push_str("{}");
                return;
            }
            out.push_str("{\n");
            for (i, (key, val)) in fields.iter().enumerate() {
                for _ in 0..indent + 1 {
                    out.push_str(indent_str);
                }
                write_json_string(out, key);
                out.push_str(": ");
                write_value_pretty(out, val, indent + 1);
                if i < fields.len() - 1 {
                    out.push(',');
                }
                out.push('\n');
            }
            for _ in 0..indent {
                out.push_str(indent_str);
            }
            out.push('}');
        }
    }
}

fn write_json_string(out: &mut String, s: &str) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000C}' => out.push_str("\\f"),
            c if c < '\u{0020}' => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

// ─── Scene Definition ─────────────────────────────────────────────────────────

/// A component's data in a scene entity.
#[derive(Clone, Debug, PartialEq)]
pub enum ComponentData {
    /// Transform component with position, rotation (quaternion), and scale.
    Transform {
        position: [f32; 3],
        rotation: [f32; 4],
        scale: [f32; 3],
    },
    /// Reference to an external asset by path.
    Asset(String),
    /// Arbitrary JSON data for custom components.
    Custom(JsonValue),
}

/// An entity descriptor in a scene.
#[derive(Clone, Debug, PartialEq)]
pub struct EntityDescriptor {
    pub name: String,
    pub components: Vec<(String, ComponentData)>,
}

/// A scene definition: a collection of entity descriptors.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneDefinition {
    pub entities: Vec<EntityDescriptor>,
}

impl SceneDefinition {
    /// Parse a scene from a JSON string.
    pub fn from_json(input: &str) -> Result<SceneDefinition, String> {
        let root = parse_json(input)?;
        let arr = root
            .as_array()
            .ok_or("scene root must be a JSON array")?;

        let mut entities = Vec::with_capacity(arr.len());
        for (i, item) in arr.iter().enumerate() {
            let _obj = item
                .as_object()
                .ok_or_else(|| format!("entity {} must be a JSON object", i))?;

            let name = item
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| format!("entity {} missing 'name' string", i))?
                .to_string();

            let components_val = item
                .get("components")
                .ok_or_else(|| format!("entity {} missing 'components'", i))?;
            let comp_fields = components_val
                .as_object()
                .ok_or_else(|| format!("entity {} 'components' must be an object", i))?;

            let mut components = Vec::with_capacity(comp_fields.len());
            for (key, val) in comp_fields {
                let data = parse_component_data(key, val)?;
                components.push((key.clone(), data));
            }

            entities.push(EntityDescriptor { name, components });
        }

        Ok(SceneDefinition { entities })
    }

    /// Serialize the scene to a JSON string.
    pub fn to_json(&self) -> String {
        let value = self.to_json_value();
        write_json(&value)
    }

    /// Serialize the scene to a pretty-printed JSON string.
    pub fn to_json_pretty(&self) -> String {
        let value = self.to_json_value();
        write_json_pretty(&value)
    }

    fn to_json_value(&self) -> JsonValue {
        let entities: Vec<JsonValue> = self
            .entities
            .iter()
            .map(|e| {
                let components: Vec<(String, JsonValue)> = e
                    .components
                    .iter()
                    .map(|(k, v)| (k.clone(), component_data_to_json(v)))
                    .collect();

                JsonValue::Object(vec![
                    ("name".into(), JsonValue::Str(e.name.clone())),
                    ("components".into(), JsonValue::Object(components)),
                ])
            })
            .collect();

        JsonValue::Array(entities)
    }
}

fn parse_component_data(key: &str, val: &JsonValue) -> Result<ComponentData, String> {
    match key {
        "transform" => {
            let obj = val
                .as_object()
                .ok_or("transform must be an object")?;

            let position = parse_f32_array_3(
                val.get("position")
                    .ok_or("transform missing 'position'")?,
            )?;
            let rotation = parse_f32_array_4(
                val.get("rotation")
                    .ok_or("transform missing 'rotation'")?,
            )?;
            let scale = parse_f32_array_3(
                val.get("scale")
                    .ok_or("transform missing 'scale'")?,
            )?;

            let _ = obj; // used above via val.get
            Ok(ComponentData::Transform {
                position,
                rotation,
                scale,
            })
        }
        "mesh" | "texture" | "material" | "audio" | "script" => {
            // Asset reference.
            match val {
                JsonValue::Str(path) => Ok(ComponentData::Asset(path.clone())),
                _ => Ok(ComponentData::Custom(val.clone())),
            }
        }
        _ => Ok(ComponentData::Custom(val.clone())),
    }
}

fn component_data_to_json(data: &ComponentData) -> JsonValue {
    match data {
        ComponentData::Transform {
            position,
            rotation,
            scale,
        } => JsonValue::Object(vec![
            (
                "position".into(),
                JsonValue::Array(position.iter().map(|&v| JsonValue::Number(v as f64)).collect()),
            ),
            (
                "rotation".into(),
                JsonValue::Array(rotation.iter().map(|&v| JsonValue::Number(v as f64)).collect()),
            ),
            (
                "scale".into(),
                JsonValue::Array(scale.iter().map(|&v| JsonValue::Number(v as f64)).collect()),
            ),
        ]),
        ComponentData::Asset(path) => JsonValue::Str(path.clone()),
        ComponentData::Custom(val) => val.clone(),
    }
}

fn parse_f32_array_3(val: &JsonValue) -> Result<[f32; 3], String> {
    let arr = val.as_array().ok_or("expected array of 3 floats")?;
    if arr.len() != 3 {
        return Err(format!("expected 3 elements, got {}", arr.len()));
    }
    Ok([
        arr[0].as_f64().ok_or("expected number")? as f32,
        arr[1].as_f64().ok_or("expected number")? as f32,
        arr[2].as_f64().ok_or("expected number")? as f32,
    ])
}

fn parse_f32_array_4(val: &JsonValue) -> Result<[f32; 4], String> {
    let arr = val.as_array().ok_or("expected array of 4 floats")?;
    if arr.len() != 4 {
        return Err(format!("expected 4 elements, got {}", arr.len()));
    }
    Ok([
        arr[0].as_f64().ok_or("expected number")? as f32,
        arr[1].as_f64().ok_or("expected number")? as f32,
        arr[2].as_f64().ok_or("expected number")? as f32,
        arr[3].as_f64().ok_or("expected number")? as f32,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── JSON parser tests ──

    #[test]
    fn parse_null() {
        assert_eq!(parse_json("null").unwrap(), JsonValue::Null);
    }

    #[test]
    fn parse_bool() {
        assert_eq!(parse_json("true").unwrap(), JsonValue::Bool(true));
        assert_eq!(parse_json("false").unwrap(), JsonValue::Bool(false));
    }

    #[test]
    fn parse_numbers() {
        assert_eq!(parse_json("42").unwrap(), JsonValue::Number(42.0));
        assert_eq!(parse_json("-3.14").unwrap(), JsonValue::Number(-3.14));
        assert_eq!(parse_json("1e10").unwrap(), JsonValue::Number(1e10));
        assert_eq!(parse_json("2.5E-3").unwrap(), JsonValue::Number(2.5e-3));
    }

    #[test]
    fn parse_strings() {
        assert_eq!(
            parse_json(r#""hello""#).unwrap(),
            JsonValue::Str("hello".into())
        );
        assert_eq!(
            parse_json(r#""line\nbreak""#).unwrap(),
            JsonValue::Str("line\nbreak".into())
        );
        assert_eq!(
            parse_json(r#""tab\there""#).unwrap(),
            JsonValue::Str("tab\there".into())
        );
        assert_eq!(
            parse_json(r#""esc\\slash""#).unwrap(),
            JsonValue::Str("esc\\slash".into())
        );
        assert_eq!(
            parse_json(r#""quote\"inside""#).unwrap(),
            JsonValue::Str("quote\"inside".into())
        );
    }

    #[test]
    fn parse_unicode_escape() {
        assert_eq!(
            parse_json(r#""\u0041""#).unwrap(),
            JsonValue::Str("A".into())
        );
    }

    #[test]
    fn parse_array() {
        let val = parse_json("[1, 2, 3]").unwrap();
        assert_eq!(
            val,
            JsonValue::Array(vec![
                JsonValue::Number(1.0),
                JsonValue::Number(2.0),
                JsonValue::Number(3.0),
            ])
        );
    }

    #[test]
    fn parse_empty_array() {
        assert_eq!(parse_json("[]").unwrap(), JsonValue::Array(vec![]));
    }

    #[test]
    fn parse_object() {
        let val = parse_json(r#"{"a": 1, "b": "two"}"#).unwrap();
        assert_eq!(
            val,
            JsonValue::Object(vec![
                ("a".into(), JsonValue::Number(1.0)),
                ("b".into(), JsonValue::Str("two".into())),
            ])
        );
    }

    #[test]
    fn parse_nested() {
        let val = parse_json(r#"{"arr": [1, {"nested": true}], "x": null}"#).unwrap();
        let obj = val.as_object().unwrap();
        assert_eq!(obj.len(), 2);
        let arr = val.get("arr").unwrap().as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[1].get("nested").unwrap(), &JsonValue::Bool(true));
        assert_eq!(val.get("x").unwrap(), &JsonValue::Null);
    }

    // ── JSON writer tests ──

    #[test]
    fn write_json_roundtrip() {
        let original = r#"{"name":"test","values":[1,2.5,true,null,"hello"]}"#;
        let parsed = parse_json(original).unwrap();
        let written = write_json(&parsed);
        let reparsed = parse_json(&written).unwrap();
        assert_eq!(parsed, reparsed);
    }

    #[test]
    fn write_json_string_escapes() {
        let val = JsonValue::Str("line\nbreak\ttab\"quote\\back".into());
        let json = write_json(&val);
        assert_eq!(json, r#""line\nbreak\ttab\"quote\\back""#);
        let reparsed = parse_json(&json).unwrap();
        assert_eq!(reparsed, val);
    }

    // ── Scene roundtrip tests ──

    #[test]
    fn scene_roundtrip() {
        let scene = SceneDefinition {
            entities: vec![
                EntityDescriptor {
                    name: "player".into(),
                    components: vec![
                        (
                            "transform".into(),
                            ComponentData::Transform {
                                position: [1.0, 2.0, 3.0],
                                rotation: [0.0, 0.0, 0.0, 1.0],
                                scale: [1.0, 1.0, 1.0],
                            },
                        ),
                        ("mesh".into(), ComponentData::Asset("meshes/player.obj".into())),
                        (
                            "texture".into(),
                            ComponentData::Asset("textures/player.png".into()),
                        ),
                    ],
                },
                EntityDescriptor {
                    name: "light".into(),
                    components: vec![
                        (
                            "transform".into(),
                            ComponentData::Transform {
                                position: [10.0, 20.0, 30.0],
                                rotation: [0.0, 0.707, 0.0, 0.707],
                                scale: [1.0, 1.0, 1.0],
                            },
                        ),
                        (
                            "intensity".into(),
                            ComponentData::Custom(JsonValue::Number(0.8)),
                        ),
                        (
                            "color".into(),
                            ComponentData::Custom(JsonValue::Array(vec![
                                JsonValue::Number(1.0),
                                JsonValue::Number(0.9),
                                JsonValue::Number(0.7),
                            ])),
                        ),
                    ],
                },
            ],
        };

        let json = scene.to_json();
        let loaded = SceneDefinition::from_json(&json).unwrap();

        assert_eq!(loaded.entities.len(), scene.entities.len());
        for (orig, loaded) in scene.entities.iter().zip(loaded.entities.iter()) {
            assert_eq!(orig.name, loaded.name);
            assert_eq!(orig.components.len(), loaded.components.len());
            for ((ok, ov), (lk, lv)) in orig.components.iter().zip(loaded.components.iter()) {
                assert_eq!(ok, lk, "component key mismatch");
                match (ov, lv) {
                    (
                        ComponentData::Transform {
                            position: op,
                            rotation: or,
                            scale: os,
                        },
                        ComponentData::Transform {
                            position: lp,
                            rotation: lr,
                            scale: ls,
                        },
                    ) => {
                        for i in 0..3 {
                            assert!(
                                (op[i] - lp[i]).abs() < 1e-6,
                                "position[{}] mismatch: {} vs {}",
                                i,
                                op[i],
                                lp[i]
                            );
                            assert!(
                                (os[i] - ls[i]).abs() < 1e-6,
                                "scale[{}] mismatch",
                                i
                            );
                        }
                        for i in 0..4 {
                            assert!(
                                (or[i] - lr[i]).abs() < 1e-6,
                                "rotation[{}] mismatch: {} vs {}",
                                i,
                                or[i],
                                lr[i]
                            );
                        }
                    }
                    (ComponentData::Asset(a), ComponentData::Asset(b)) => {
                        assert_eq!(a, b);
                    }
                    (ComponentData::Custom(a), ComponentData::Custom(b)) => {
                        assert_eq!(a, b);
                    }
                    _ => panic!("component data type mismatch for key '{}'", ok),
                }
            }
        }
    }

    #[test]
    fn scene_pretty_roundtrip() {
        let scene = SceneDefinition {
            entities: vec![EntityDescriptor {
                name: "test_entity".into(),
                components: vec![
                    (
                        "transform".into(),
                        ComponentData::Transform {
                            position: [0.0, 0.0, 0.0],
                            rotation: [0.0, 0.0, 0.0, 1.0],
                            scale: [2.0, 2.0, 2.0],
                        },
                    ),
                    ("mesh".into(), ComponentData::Asset("cube.obj".into())),
                ],
            }],
        };

        let pretty = scene.to_json_pretty();
        let loaded = SceneDefinition::from_json(&pretty).unwrap();
        assert_eq!(loaded.entities.len(), 1);
        assert_eq!(loaded.entities[0].name, "test_entity");
        assert_eq!(loaded.entities[0].components.len(), 2);
    }

    #[test]
    fn scene_from_json_string() {
        let json = r#"[
            {
                "name": "camera",
                "components": {
                    "transform": {
                        "position": [0, 5, -10],
                        "rotation": [0, 0, 0, 1],
                        "scale": [1, 1, 1]
                    },
                    "fov": 60,
                    "near": 0.1,
                    "far": 1000
                }
            }
        ]"#;

        let scene = SceneDefinition::from_json(json).unwrap();
        assert_eq!(scene.entities.len(), 1);
        assert_eq!(scene.entities[0].name, "camera");
        assert_eq!(scene.entities[0].components.len(), 4);

        // transform
        match &scene.entities[0].components[0].1 {
            ComponentData::Transform { position, .. } => {
                assert_eq!(*position, [0.0, 5.0, -10.0]);
            }
            _ => panic!("expected transform"),
        }

        // fov is custom
        assert_eq!(scene.entities[0].components[1].0, "fov");
        match &scene.entities[0].components[1].1 {
            ComponentData::Custom(JsonValue::Number(n)) => {
                assert!((n - 60.0).abs() < 0.001);
            }
            other => panic!("expected custom number for fov, got {:?}", other),
        }
    }
}
