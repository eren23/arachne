use arachne_ecs::Entity;
use arachne_math::Transform;
use std::collections::HashMap;

use crate::graph::SceneGraph;
use crate::transform_prop::TransformPropagation;

// SERIALIZED_VALUE ------

#[derive(Clone, Debug, PartialEq)]
pub enum SerializedValue {
    Null,
    Bool(bool),
    Number(f64),
    Str(String),
    Array(Vec<SerializedValue>),
    Object(Vec<(String, SerializedValue)>),
}

// COMPONENT_ENTRY ------

#[derive(Clone, Debug, PartialEq)]
pub struct ComponentEntry {
    pub name: String,
    pub data: SerializedValue,
}

// SERIALIZED_ENTITY ------

#[derive(Clone, Debug, PartialEq)]
pub struct SerializedEntity {
    pub id: u32,
    pub parent: Option<u32>,
    pub components: Vec<ComponentEntry>,
}

// SERIALIZED_SCENE ------

#[derive(Clone, Debug, PartialEq)]
pub struct SerializedScene {
    pub entities: Vec<SerializedEntity>,
}

// COMPONENT_REGISTRY ------

struct SerializeFns {
    serialize_fn: Box<dyn Fn(&dyn std::any::Any) -> SerializedValue + Send + Sync>,
    deserialize_fn: Box<dyn Fn(&SerializedValue) -> Option<Box<dyn std::any::Any + Send + Sync>> + Send + Sync>,
}

pub struct ComponentRegistry {
    entries: HashMap<String, SerializeFns>,
}

impl ComponentRegistry {
    #[inline]
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn register<T: 'static + Send + Sync>(
        &mut self,
        name: &str,
        serialize_fn: fn(&T) -> SerializedValue,
        deserialize_fn: fn(&SerializedValue) -> Option<T>,
    ) {
        let ser = move |val: &dyn std::any::Any| -> SerializedValue {
            if let Some(v) = val.downcast_ref::<T>() {
                serialize_fn(v)
            } else {
                SerializedValue::Null
            }
        };
        let deser = move |val: &SerializedValue| -> Option<Box<dyn std::any::Any + Send + Sync>> {
            deserialize_fn(val).map(|v| Box::new(v) as Box<dyn std::any::Any + Send + Sync>)
        };
        self.entries.insert(name.to_string(), SerializeFns {
            serialize_fn: Box::new(ser),
            deserialize_fn: Box::new(deser),
        });
    }

    #[inline]
    pub fn has(&self, name: &str) -> bool {
        self.entries.contains_key(name)
    }
}

impl Default for ComponentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// SERIALIZE_SCENE ------

fn serialize_transform(t: &Transform) -> SerializedValue {
    SerializedValue::Object(vec![
        ("px".to_string(), SerializedValue::Number(t.position.x as f64)),
        ("py".to_string(), SerializedValue::Number(t.position.y as f64)),
        ("pz".to_string(), SerializedValue::Number(t.position.z as f64)),
        ("rx".to_string(), SerializedValue::Number(t.rotation.x as f64)),
        ("ry".to_string(), SerializedValue::Number(t.rotation.y as f64)),
        ("rz".to_string(), SerializedValue::Number(t.rotation.z as f64)),
        ("rw".to_string(), SerializedValue::Number(t.rotation.w as f64)),
        ("sx".to_string(), SerializedValue::Number(t.scale.x as f64)),
        ("sy".to_string(), SerializedValue::Number(t.scale.y as f64)),
        ("sz".to_string(), SerializedValue::Number(t.scale.z as f64)),
    ])
}

fn deserialize_transform(val: &SerializedValue) -> Option<Transform> {
    if let SerializedValue::Object(fields) = val {
        let mut map = HashMap::new();
        for (k, v) in fields {
            if let SerializedValue::Number(n) = v {
                map.insert(k.as_str(), *n as f32);
            }
        }
        Some(Transform::new(
            arachne_math::Vec3::new(
                *map.get("px").unwrap_or(&0.0),
                *map.get("py").unwrap_or(&0.0),
                *map.get("pz").unwrap_or(&0.0),
            ),
            arachne_math::Quat::new(
                *map.get("rx").unwrap_or(&0.0),
                *map.get("ry").unwrap_or(&0.0),
                *map.get("rz").unwrap_or(&0.0),
                *map.get("rw").unwrap_or(&1.0),
            ),
            arachne_math::Vec3::new(
                *map.get("sx").unwrap_or(&1.0),
                *map.get("sy").unwrap_or(&1.0),
                *map.get("sz").unwrap_or(&1.0),
            ),
        ))
    } else {
        None
    }
}

pub fn serialize_scene(
    graph: &SceneGraph,
    transforms: &TransformPropagation,
    entities: &[Entity],
    _registry: &ComponentRegistry,
) -> SerializedScene {
    let mut serialized = Vec::new();
    for &entity in entities {
        let parent = graph.parent_of(entity).map(|p| p.index());
        let mut components = Vec::new();

        if let Some(t) = transforms.local_transform(entity) {
            components.push(ComponentEntry {
                name: "Transform".to_string(),
                data: serialize_transform(&t),
            });
        }

        serialized.push(SerializedEntity {
            id: entity.index(),
            parent,
            components,
        });
    }
    SerializedScene { entities: serialized }
}

// DESERIALIZE_SCENE ------

pub fn deserialize_scene(
    scene: &SerializedScene,
    graph: &mut SceneGraph,
    transforms: &mut TransformPropagation,
    _registry: &ComponentRegistry,
) -> Vec<Entity> {
    let mut id_to_entity: HashMap<u32, Entity> = HashMap::new();
    let mut created = Vec::new();

    // First pass: create all entities
    for se in &scene.entities {
        let entity = Entity::from_raw(se.id, 0);
        id_to_entity.insert(se.id, entity);
        created.push(entity);

        // Deserialize components
        for comp in &se.components {
            if comp.name == "Transform" {
                if let Some(t) = deserialize_transform(&comp.data) {
                    transforms.set_local(entity, t);
                }
            }
        }
    }

    // Second pass: set up parent-child relationships
    for se in &scene.entities {
        if let Some(parent_id) = se.parent {
            if let (Some(&child), Some(&parent)) = (id_to_entity.get(&se.id), id_to_entity.get(&parent_id)) {
                graph.add_child(parent, child);
            }
        }
    }

    created
}

// JSON_OUTPUT ------

pub fn scene_to_json(scene: &SerializedScene) -> String {
    let mut out = String::new();
    out.push_str("{\"entities\":[");
    for (i, entity) in scene.entities.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str("{\"id\":");
        out.push_str(&entity.id.to_string());
        out.push_str(",\"parent\":");
        match entity.parent {
            Some(p) => out.push_str(&p.to_string()),
            None => out.push_str("null"),
        }
        out.push_str(",\"components\":[");
        for (j, comp) in entity.components.iter().enumerate() {
            if j > 0 {
                out.push(',');
            }
            out.push_str("{\"name\":");
            write_json_string(&mut out, &comp.name);
            out.push_str(",\"data\":");
            write_json_value(&mut out, &comp.data);
            out.push('}');
        }
        out.push_str("]}");
    }
    out.push_str("]}");
    out
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
            c => out.push(c),
        }
    }
    out.push('"');
}

fn write_json_value(out: &mut String, val: &SerializedValue) {
    match val {
        SerializedValue::Null => out.push_str("null"),
        SerializedValue::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        SerializedValue::Number(n) => {
            // Ensure integer values don't have unnecessary decimal points
            if *n == (*n as i64) as f64 && n.is_finite() {
                out.push_str(&(*n as i64).to_string());
            } else {
                out.push_str(&format!("{}", n));
            }
        }
        SerializedValue::Str(s) => write_json_string(out, s),
        SerializedValue::Array(arr) => {
            out.push('[');
            for (i, v) in arr.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_json_value(out, v);
            }
            out.push(']');
        }
        SerializedValue::Object(fields) => {
            out.push('{');
            for (i, (k, v)) in fields.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_json_string(out, k);
                out.push(':');
                write_json_value(out, v);
            }
            out.push('}');
        }
    }
}

// JSON_PARSER ------

pub fn scene_from_json(json: &str) -> Result<SerializedScene, String> {
    let mut parser = JsonParser::new(json);
    parser.parse_scene()
}

struct JsonParser<'a> {
    input: &'a [u8],
    pos: usize,
}

impl<'a> JsonParser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input: input.as_bytes(),
            pos: 0,
        }
    }

    fn skip_ws(&mut self) {
        while self.pos < self.input.len() {
            match self.input[self.pos] {
                b' ' | b'\n' | b'\r' | b'\t' => self.pos += 1,
                _ => break,
            }
        }
    }

    fn peek(&mut self) -> Result<u8, String> {
        self.skip_ws();
        if self.pos < self.input.len() {
            Ok(self.input[self.pos])
        } else {
            Err("Unexpected end of input".to_string())
        }
    }

    fn expect(&mut self, ch: u8) -> Result<(), String> {
        self.skip_ws();
        if self.pos < self.input.len() && self.input[self.pos] == ch {
            self.pos += 1;
            Ok(())
        } else {
            Err(format!(
                "Expected '{}' at position {}, got '{}'",
                ch as char,
                self.pos,
                if self.pos < self.input.len() {
                    self.input[self.pos] as char
                } else {
                    '\0'
                }
            ))
        }
    }

    fn parse_string(&mut self) -> Result<String, String> {
        self.skip_ws();
        self.expect(b'"')?;
        let mut s = String::new();
        while self.pos < self.input.len() {
            let ch = self.input[self.pos];
            self.pos += 1;
            if ch == b'"' {
                return Ok(s);
            } else if ch == b'\\' {
                if self.pos >= self.input.len() {
                    return Err("Unterminated escape".to_string());
                }
                let esc = self.input[self.pos];
                self.pos += 1;
                match esc {
                    b'"' => s.push('"'),
                    b'\\' => s.push('\\'),
                    b'n' => s.push('\n'),
                    b'r' => s.push('\r'),
                    b't' => s.push('\t'),
                    b'/' => s.push('/'),
                    _ => {
                        s.push('\\');
                        s.push(esc as char);
                    }
                }
            } else {
                s.push(ch as char);
            }
        }
        Err("Unterminated string".to_string())
    }

    fn parse_number(&mut self) -> Result<f64, String> {
        self.skip_ws();
        let start = self.pos;
        if self.pos < self.input.len() && self.input[self.pos] == b'-' {
            self.pos += 1;
        }
        while self.pos < self.input.len() && self.input[self.pos].is_ascii_digit() {
            self.pos += 1;
        }
        if self.pos < self.input.len() && self.input[self.pos] == b'.' {
            self.pos += 1;
            while self.pos < self.input.len() && self.input[self.pos].is_ascii_digit() {
                self.pos += 1;
            }
        }
        // Handle exponent
        if self.pos < self.input.len() && (self.input[self.pos] == b'e' || self.input[self.pos] == b'E') {
            self.pos += 1;
            if self.pos < self.input.len() && (self.input[self.pos] == b'+' || self.input[self.pos] == b'-') {
                self.pos += 1;
            }
            while self.pos < self.input.len() && self.input[self.pos].is_ascii_digit() {
                self.pos += 1;
            }
        }
        let s = std::str::from_utf8(&self.input[start..self.pos])
            .map_err(|e| e.to_string())?;
        s.parse::<f64>().map_err(|e| e.to_string())
    }

    fn parse_value(&mut self) -> Result<SerializedValue, String> {
        let ch = self.peek()?;
        match ch {
            b'"' => Ok(SerializedValue::Str(self.parse_string()?)),
            b'{' => self.parse_object(),
            b'[' => self.parse_array(),
            b't' => {
                self.expect_literal("true")?;
                Ok(SerializedValue::Bool(true))
            }
            b'f' => {
                self.expect_literal("false")?;
                Ok(SerializedValue::Bool(false))
            }
            b'n' => {
                self.expect_literal("null")?;
                Ok(SerializedValue::Null)
            }
            _ => Ok(SerializedValue::Number(self.parse_number()?)),
        }
    }

    fn expect_literal(&mut self, lit: &str) -> Result<(), String> {
        self.skip_ws();
        let bytes = lit.as_bytes();
        if self.pos + bytes.len() > self.input.len() {
            return Err(format!("Expected '{}'", lit));
        }
        for (i, &b) in bytes.iter().enumerate() {
            if self.input[self.pos + i] != b {
                return Err(format!("Expected '{}'", lit));
            }
        }
        self.pos += bytes.len();
        Ok(())
    }

    fn parse_object(&mut self) -> Result<SerializedValue, String> {
        self.expect(b'{')?;
        let mut fields = Vec::new();
        if self.peek()? != b'}' {
            loop {
                let key = self.parse_string()?;
                self.expect(b':')?;
                let val = self.parse_value()?;
                fields.push((key, val));
                if self.peek()? == b',' {
                    self.pos += 1;
                } else {
                    break;
                }
            }
        }
        self.expect(b'}')?;
        Ok(SerializedValue::Object(fields))
    }

    fn parse_array(&mut self) -> Result<SerializedValue, String> {
        self.expect(b'[')?;
        let mut items = Vec::new();
        if self.peek()? != b']' {
            loop {
                items.push(self.parse_value()?);
                if self.peek()? == b',' {
                    self.pos += 1;
                } else {
                    break;
                }
            }
        }
        self.expect(b']')?;
        Ok(SerializedValue::Array(items))
    }

    fn parse_scene(&mut self) -> Result<SerializedScene, String> {
        self.expect(b'{')?;
        let key = self.parse_string()?;
        if key != "entities" {
            return Err(format!("Expected 'entities' key, got '{}'", key));
        }
        self.expect(b':')?;
        self.expect(b'[')?;

        let mut entities = Vec::new();
        if self.peek()? != b']' {
            loop {
                entities.push(self.parse_serialized_entity()?);
                if self.peek()? == b',' {
                    self.pos += 1;
                } else {
                    break;
                }
            }
        }
        self.expect(b']')?;
        self.expect(b'}')?;
        Ok(SerializedScene { entities })
    }

    fn parse_serialized_entity(&mut self) -> Result<SerializedEntity, String> {
        self.expect(b'{')?;
        let mut id = 0u32;
        let mut parent: Option<u32> = None;
        let mut components = Vec::new();

        loop {
            let key = self.parse_string()?;
            self.expect(b':')?;
            match key.as_str() {
                "id" => {
                    let n = self.parse_number()?;
                    id = n as u32;
                }
                "parent" => {
                    if self.peek()? == b'n' {
                        self.expect_literal("null")?;
                        parent = None;
                    } else {
                        let n = self.parse_number()?;
                        parent = Some(n as u32);
                    }
                }
                "components" => {
                    self.expect(b'[')?;
                    if self.peek()? != b']' {
                        loop {
                            components.push(self.parse_component_entry()?);
                            if self.peek()? == b',' {
                                self.pos += 1;
                            } else {
                                break;
                            }
                        }
                    }
                    self.expect(b']')?;
                }
                _ => {
                    // Skip unknown fields
                    self.parse_value()?;
                }
            }
            if self.peek()? == b',' {
                self.pos += 1;
            } else {
                break;
            }
        }

        self.expect(b'}')?;
        Ok(SerializedEntity { id, parent, components })
    }

    fn parse_component_entry(&mut self) -> Result<ComponentEntry, String> {
        self.expect(b'{')?;
        let mut name = String::new();
        let mut data = SerializedValue::Null;

        loop {
            let key = self.parse_string()?;
            self.expect(b':')?;
            match key.as_str() {
                "name" => name = self.parse_string()?,
                "data" => data = self.parse_value()?,
                _ => { self.parse_value()?; }
            }
            if self.peek()? == b',' {
                self.pos += 1;
            } else {
                break;
            }
        }

        self.expect(b'}')?;
        Ok(ComponentEntry { name, data })
    }
}

// TESTS ------

#[cfg(test)]
mod tests {
    use super::*;
    use arachne_math::{Quat, Vec3};

    const EPSILON: f32 = 1e-5;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < EPSILON
    }

    fn e(index: u32) -> Entity {
        Entity::from_raw(index, 0)
    }

    #[test]
    fn roundtrip_100_entities() {
        let mut graph = SceneGraph::new();
        let mut tp = TransformPropagation::new();
        let registry = ComponentRegistry::new();

        // Create 100 entities in a chain
        let mut entities = Vec::new();
        for i in 0..100u32 {
            let entity = e(i);
            entities.push(entity);
            let transform = Transform::new(
                Vec3::new(i as f32, (i as f32) * 0.5, 0.0),
                Quat::IDENTITY,
                Vec3::ONE,
            );
            tp.set_local(entity, transform);
            if i > 0 {
                graph.add_child(e(i - 1), entity);
            }
        }

        // Serialize
        let scene = serialize_scene(&graph, &tp, &entities, &registry);
        assert_eq!(scene.entities.len(), 100);

        // Convert to JSON and back
        let json = scene_to_json(&scene);
        let parsed = scene_from_json(&json).expect("Failed to parse JSON");
        assert_eq!(parsed.entities.len(), 100);

        // Verify structure matches
        for (orig, parsed_e) in scene.entities.iter().zip(parsed.entities.iter()) {
            assert_eq!(orig.id, parsed_e.id);
            assert_eq!(orig.parent, parsed_e.parent);
            assert_eq!(orig.components.len(), parsed_e.components.len());
        }

        // Deserialize into a new graph
        let mut graph2 = SceneGraph::new();
        let mut tp2 = TransformPropagation::new();
        let deserialized = deserialize_scene(&parsed, &mut graph2, &mut tp2, &registry);
        assert_eq!(deserialized.len(), 100);

        // Verify parent-child relationships match
        for i in 1..100u32 {
            let child = e(i);
            let expected_parent = e(i - 1);
            assert_eq!(
                graph2.parent_of(child),
                Some(expected_parent),
                "Entity {} should have parent {}",
                i, i - 1,
            );
        }
        assert_eq!(graph2.parent_of(e(0)), None);

        // Verify transforms roundtrip
        for i in 0..100u32 {
            let entity = e(i);
            let orig_t = tp.local_transform(entity).unwrap();
            let new_t = tp2.local_transform(entity).unwrap();
            assert!(
                approx_eq(orig_t.position.x, new_t.position.x)
                    && approx_eq(orig_t.position.y, new_t.position.y)
                    && approx_eq(orig_t.position.z, new_t.position.z),
                "Transform position mismatch for entity {}",
                i,
            );
        }
    }

    #[test]
    fn json_value_roundtrip() {
        let val = SerializedValue::Object(vec![
            ("name".to_string(), SerializedValue::Str("test".to_string())),
            ("count".to_string(), SerializedValue::Number(42.0)),
            ("active".to_string(), SerializedValue::Bool(true)),
            ("data".to_string(), SerializedValue::Null),
            ("items".to_string(), SerializedValue::Array(vec![
                SerializedValue::Number(1.0),
                SerializedValue::Number(2.0),
                SerializedValue::Number(3.0),
            ])),
        ]);

        let mut out = String::new();
        write_json_value(&mut out, &val);

        let mut parser = JsonParser::new(&out);
        let parsed = parser.parse_value().unwrap();
        assert_eq!(val, parsed);
    }

    #[test]
    fn serialize_empty_scene() {
        let graph = SceneGraph::new();
        let tp = TransformPropagation::new();
        let registry = ComponentRegistry::new();
        let entities: Vec<Entity> = Vec::new();

        let scene = serialize_scene(&graph, &tp, &entities, &registry);
        assert!(scene.entities.is_empty());

        let json = scene_to_json(&scene);
        let parsed = scene_from_json(&json).unwrap();
        assert!(parsed.entities.is_empty());
    }

    #[test]
    fn json_string_escaping() {
        let mut out = String::new();
        write_json_string(&mut out, "hello \"world\"\nnewline");
        assert_eq!(out, r#""hello \"world\"\nnewline""#);

        let mut parser = JsonParser::new(&out);
        let s = parser.parse_string().unwrap();
        assert_eq!(s, "hello \"world\"\nnewline");
    }

    #[test]
    fn transform_serialize_deserialize() {
        let t = Transform::new(
            Vec3::new(1.0, 2.0, 3.0),
            Quat::from_axis_angle(Vec3::Y, core::f32::consts::FRAC_PI_4),
            Vec3::new(2.0, 2.0, 2.0),
        );

        let val = serialize_transform(&t);
        let t2 = deserialize_transform(&val).unwrap();

        assert!(approx_eq(t.position.x, t2.position.x));
        assert!(approx_eq(t.position.y, t2.position.y));
        assert!(approx_eq(t.position.z, t2.position.z));
        assert!(approx_eq(t.rotation.x, t2.rotation.x));
        assert!(approx_eq(t.rotation.y, t2.rotation.y));
        assert!(approx_eq(t.rotation.z, t2.rotation.z));
        assert!(approx_eq(t.rotation.w, t2.rotation.w));
        assert!(approx_eq(t.scale.x, t2.scale.x));
        assert!(approx_eq(t.scale.y, t2.scale.y));
        assert!(approx_eq(t.scale.z, t2.scale.z));
    }
}
