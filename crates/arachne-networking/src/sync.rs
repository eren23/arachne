//! State synchronization: snapshots, deltas, and networked component traits.
//!
//! Provides a system for capturing world state as snapshots, computing
//! deltas between snapshots, and applying deltas to reconstruct state.
//! Designed for server-authoritative networking where the server sends
//! either full snapshots or compressed deltas.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use alloc::vec;

extern crate alloc;

/// A unique identifier for an entity in the network protocol.
/// Server and client may map these differently.
pub type NetEntityId = u32;

/// A unique identifier for a component type.
pub type ComponentTypeId = u16;

// ---------------------------------------------------------------------------
// ComponentData
// ---------------------------------------------------------------------------

/// Raw serialized component data.
///
/// Components are stored as opaque byte vectors to avoid serde dependency.
/// Each component type is responsible for its own serialization via the
/// `NetworkedComponent` trait.
#[derive(Clone, Debug, PartialEq)]
pub struct ComponentData {
    pub type_id: ComponentTypeId,
    pub data: Vec<u8>,
}

impl ComponentData {
    /// Creates new component data.
    pub fn new(type_id: ComponentTypeId, data: Vec<u8>) -> Self {
        Self { type_id, data }
    }

    /// Encodes this component data into bytes.
    ///
    /// Format: type_id(u16 LE) + data_len(u16 LE) + data
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(4 + self.data.len());
        buf.extend_from_slice(&self.type_id.to_le_bytes());
        buf.extend_from_slice(&(self.data.len() as u16).to_le_bytes());
        buf.extend_from_slice(&self.data);
        buf
    }

    /// Decodes component data from bytes. Returns (ComponentData, bytes_consumed).
    pub fn decode(buf: &[u8]) -> Option<(Self, usize)> {
        if buf.len() < 4 {
            return None;
        }
        let type_id = u16::from_le_bytes([buf[0], buf[1]]);
        let data_len = u16::from_le_bytes([buf[2], buf[3]]) as usize;
        if buf.len() < 4 + data_len {
            return None;
        }
        let data = buf[4..4 + data_len].to_vec();
        Some((Self { type_id, data }, 4 + data_len))
    }
}

// ---------------------------------------------------------------------------
// NetworkedComponent trait
// ---------------------------------------------------------------------------

/// Trait for components that can be serialized/deserialized for network sync.
///
/// Implementors provide manual serialization to keep WASM size small
/// (no serde dependency).
pub trait NetworkedComponent {
    /// The unique type identifier for this component.
    const TYPE_ID: ComponentTypeId;

    /// Serializes this component to bytes.
    fn net_serialize(&self) -> Vec<u8>;

    /// Deserializes a component from bytes.
    fn net_deserialize(data: &[u8]) -> Option<Self>
    where
        Self: Sized;
}

// ---------------------------------------------------------------------------
// Snapshot
// ---------------------------------------------------------------------------

/// A full snapshot of the networked world state.
///
/// Maps entity IDs to their component data. Used for initial sync
/// or periodic full-state broadcasts.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    /// The server tick at which this snapshot was taken.
    pub tick: u64,
    /// Entity ID -> list of component data.
    pub entities: BTreeMap<NetEntityId, Vec<ComponentData>>,
}

impl Snapshot {
    /// Creates an empty snapshot at the given tick.
    pub fn new(tick: u64) -> Self {
        Self {
            tick,
            entities: BTreeMap::new(),
        }
    }

    /// Adds a component to an entity in the snapshot.
    pub fn add_component(&mut self, entity: NetEntityId, component: ComponentData) {
        self.entities.entry(entity).or_default().push(component);
    }

    /// Returns the number of entities in the snapshot.
    pub fn entity_count(&self) -> usize {
        self.entities.len()
    }

    /// Returns the total number of components across all entities.
    pub fn component_count(&self) -> usize {
        self.entities.values().map(|v| v.len()).sum()
    }

    /// Serializes the snapshot to bytes.
    ///
    /// Format:
    /// - tick: u64 LE (8 bytes)
    /// - entity_count: u32 LE (4 bytes)
    /// - For each entity:
    ///   - entity_id: u32 LE (4 bytes)
    ///   - component_count: u16 LE (2 bytes)
    ///   - For each component: ComponentData::encode()
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.tick.to_le_bytes());
        buf.extend_from_slice(&(self.entities.len() as u32).to_le_bytes());

        for (&entity_id, components) in &self.entities {
            buf.extend_from_slice(&entity_id.to_le_bytes());
            buf.extend_from_slice(&(components.len() as u16).to_le_bytes());
            for comp in components {
                buf.extend_from_slice(&comp.encode());
            }
        }

        buf
    }

    /// Deserializes a snapshot from bytes.
    pub fn decode(buf: &[u8]) -> Option<Self> {
        if buf.len() < 12 {
            return None;
        }
        let tick = u64::from_le_bytes(buf[0..8].try_into().ok()?);
        let entity_count = u32::from_le_bytes(buf[8..12].try_into().ok()?) as usize;

        let mut offset = 12;
        let mut entities = BTreeMap::new();

        for _ in 0..entity_count {
            if offset + 6 > buf.len() {
                return None;
            }
            let entity_id = u32::from_le_bytes(buf[offset..offset + 4].try_into().ok()?);
            offset += 4;
            let comp_count = u16::from_le_bytes(buf[offset..offset + 2].try_into().ok()?) as usize;
            offset += 2;

            let mut components = Vec::with_capacity(comp_count);
            for _ in 0..comp_count {
                let (comp, consumed) = ComponentData::decode(&buf[offset..])?;
                components.push(comp);
                offset += consumed;
            }
            entities.insert(entity_id, components);
        }

        Some(Self { tick, entities })
    }
}

// ---------------------------------------------------------------------------
// Delta
// ---------------------------------------------------------------------------

/// A change to a single component on an entity.
#[derive(Clone, Debug, PartialEq)]
pub enum FieldChange {
    /// Component was added or updated.
    Updated(ComponentData),
    /// Component was removed (identified by type_id).
    Removed(ComponentTypeId),
}

/// Represents changes to a single entity.
#[derive(Clone, Debug, PartialEq)]
pub struct EntityDelta {
    pub entity_id: NetEntityId,
    pub changes: Vec<FieldChange>,
}

/// A delta between two snapshots.
///
/// Contains only the entities and components that changed between
/// the base snapshot and the target snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct Delta {
    /// The base tick this delta is relative to.
    pub base_tick: u64,
    /// The target tick this delta produces.
    pub target_tick: u64,
    /// Per-entity changes.
    pub entity_deltas: Vec<EntityDelta>,
    /// Entities that were removed entirely.
    pub removed_entities: Vec<NetEntityId>,
}

impl Delta {
    /// Returns whether this delta has no changes.
    pub fn is_empty(&self) -> bool {
        self.entity_deltas.is_empty() && self.removed_entities.is_empty()
    }

    /// Serializes the delta to bytes.
    ///
    /// Format:
    /// - base_tick: u64 LE
    /// - target_tick: u64 LE
    /// - removed_count: u32 LE
    /// - removed entity IDs: [u32 LE; removed_count]
    /// - delta_count: u32 LE
    /// - For each entity delta:
    ///   - entity_id: u32 LE
    ///   - change_count: u16 LE
    ///   - For each change:
    ///     - tag: u8 (0 = Updated, 1 = Removed)
    ///     - if Updated: ComponentData::encode()
    ///     - if Removed: type_id (u16 LE)
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.base_tick.to_le_bytes());
        buf.extend_from_slice(&self.target_tick.to_le_bytes());

        // Removed entities
        buf.extend_from_slice(&(self.removed_entities.len() as u32).to_le_bytes());
        for &eid in &self.removed_entities {
            buf.extend_from_slice(&eid.to_le_bytes());
        }

        // Entity deltas
        buf.extend_from_slice(&(self.entity_deltas.len() as u32).to_le_bytes());
        for ed in &self.entity_deltas {
            buf.extend_from_slice(&ed.entity_id.to_le_bytes());
            buf.extend_from_slice(&(ed.changes.len() as u16).to_le_bytes());
            for change in &ed.changes {
                match change {
                    FieldChange::Updated(comp) => {
                        buf.push(0x00);
                        buf.extend_from_slice(&comp.encode());
                    }
                    FieldChange::Removed(type_id) => {
                        buf.push(0x01);
                        buf.extend_from_slice(&type_id.to_le_bytes());
                    }
                }
            }
        }

        buf
    }

    /// Deserializes a delta from bytes.
    pub fn decode(buf: &[u8]) -> Option<Self> {
        if buf.len() < 20 {
            return None;
        }
        let base_tick = u64::from_le_bytes(buf[0..8].try_into().ok()?);
        let target_tick = u64::from_le_bytes(buf[8..16].try_into().ok()?);
        let removed_count = u32::from_le_bytes(buf[16..20].try_into().ok()?) as usize;

        let mut offset = 20;
        let mut removed_entities = Vec::with_capacity(removed_count);
        for _ in 0..removed_count {
            if offset + 4 > buf.len() {
                return None;
            }
            let eid = u32::from_le_bytes(buf[offset..offset + 4].try_into().ok()?);
            removed_entities.push(eid);
            offset += 4;
        }

        if offset + 4 > buf.len() {
            return None;
        }
        let delta_count = u32::from_le_bytes(buf[offset..offset + 4].try_into().ok()?) as usize;
        offset += 4;

        let mut entity_deltas = Vec::with_capacity(delta_count);
        for _ in 0..delta_count {
            if offset + 6 > buf.len() {
                return None;
            }
            let entity_id = u32::from_le_bytes(buf[offset..offset + 4].try_into().ok()?);
            offset += 4;
            let change_count = u16::from_le_bytes(buf[offset..offset + 2].try_into().ok()?) as usize;
            offset += 2;

            let mut changes = Vec::with_capacity(change_count);
            for _ in 0..change_count {
                if offset >= buf.len() {
                    return None;
                }
                let tag = buf[offset];
                offset += 1;
                match tag {
                    0x00 => {
                        let (comp, consumed) = ComponentData::decode(&buf[offset..])?;
                        changes.push(FieldChange::Updated(comp));
                        offset += consumed;
                    }
                    0x01 => {
                        if offset + 2 > buf.len() {
                            return None;
                        }
                        let type_id = u16::from_le_bytes([buf[offset], buf[offset + 1]]);
                        changes.push(FieldChange::Removed(type_id));
                        offset += 2;
                    }
                    _ => return None,
                }
            }
            entity_deltas.push(EntityDelta { entity_id, changes });
        }

        Some(Self {
            base_tick,
            target_tick,
            entity_deltas,
            removed_entities,
        })
    }
}

// ---------------------------------------------------------------------------
// DeltaCompression
// ---------------------------------------------------------------------------

/// Utility for computing and applying deltas between snapshots.
pub struct DeltaCompression;

impl DeltaCompression {
    /// Computes a delta from `base` to `target`.
    ///
    /// Identifies:
    /// - New entities (in target but not base)
    /// - Removed entities (in base but not target)
    /// - Changed components (same entity, different component data)
    /// - Added components (new component on existing entity)
    /// - Removed components (component missing on existing entity)
    pub fn compute(base: &Snapshot, target: &Snapshot) -> Delta {
        let mut entity_deltas = Vec::new();
        let mut removed_entities = Vec::new();

        // Find entities removed in target
        for &eid in base.entities.keys() {
            if !target.entities.contains_key(&eid) {
                removed_entities.push(eid);
            }
        }

        // Find new or changed entities in target
        for (&eid, target_components) in &target.entities {
            match base.entities.get(&eid) {
                None => {
                    // New entity: all components are "updated"
                    let changes = target_components
                        .iter()
                        .map(|c| FieldChange::Updated(c.clone()))
                        .collect();
                    entity_deltas.push(EntityDelta {
                        entity_id: eid,
                        changes,
                    });
                }
                Some(base_components) => {
                    let mut changes = Vec::new();

                    // Check for updated or new components
                    for tc in target_components {
                        let base_match = base_components
                            .iter()
                            .find(|bc| bc.type_id == tc.type_id);
                        match base_match {
                            Some(bc) if bc.data != tc.data => {
                                changes.push(FieldChange::Updated(tc.clone()));
                            }
                            None => {
                                changes.push(FieldChange::Updated(tc.clone()));
                            }
                            _ => {} // unchanged
                        }
                    }

                    // Check for removed components
                    for bc in base_components {
                        let still_exists = target_components
                            .iter()
                            .any(|tc| tc.type_id == bc.type_id);
                        if !still_exists {
                            changes.push(FieldChange::Removed(bc.type_id));
                        }
                    }

                    if !changes.is_empty() {
                        entity_deltas.push(EntityDelta {
                            entity_id: eid,
                            changes,
                        });
                    }
                }
            }
        }

        Delta {
            base_tick: base.tick,
            target_tick: target.tick,
            entity_deltas,
            removed_entities,
        }
    }

    /// Applies a delta to a snapshot, producing the target snapshot.
    pub fn apply(base: &Snapshot, delta: &Delta) -> Snapshot {
        let mut result = base.clone();
        result.tick = delta.target_tick;

        // Remove entities
        for &eid in &delta.removed_entities {
            result.entities.remove(&eid);
        }

        // Apply entity deltas
        for ed in &delta.entity_deltas {
            let components = result.entities.entry(ed.entity_id).or_default();

            for change in &ed.changes {
                match change {
                    FieldChange::Updated(comp) => {
                        // Replace existing component of same type, or add new
                        if let Some(existing) = components
                            .iter_mut()
                            .find(|c| c.type_id == comp.type_id)
                        {
                            existing.data = comp.data.clone();
                        } else {
                            components.push(comp.clone());
                        }
                    }
                    FieldChange::Removed(type_id) => {
                        components.retain(|c| c.type_id != *type_id);
                    }
                }
            }

            // Clean up empty entity entries
            if components.is_empty() {
                result.entities.remove(&ed.entity_id);
            }
        }

        result
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- ComponentData tests --------------------------------------------------

    #[test]
    fn test_component_data_encode_decode() {
        let comp = ComponentData::new(42, vec![1, 2, 3, 4]);
        let encoded = comp.encode();
        let (decoded, consumed) = ComponentData::decode(&encoded).unwrap();
        assert_eq!(consumed, encoded.len());
        assert_eq!(decoded, comp);
    }

    #[test]
    fn test_component_data_empty_payload() {
        let comp = ComponentData::new(0, Vec::new());
        let encoded = comp.encode();
        let (decoded, consumed) = ComponentData::decode(&encoded).unwrap();
        assert_eq!(consumed, 4);
        assert_eq!(decoded, comp);
    }

    #[test]
    fn test_component_data_decode_too_short() {
        assert!(ComponentData::decode(&[0, 1]).is_none());
    }

    // -- Snapshot tests -------------------------------------------------------

    #[test]
    fn test_snapshot_empty() {
        let snap = Snapshot::new(10);
        assert_eq!(snap.tick, 10);
        assert_eq!(snap.entity_count(), 0);
        assert_eq!(snap.component_count(), 0);
    }

    #[test]
    fn test_snapshot_add_components() {
        let mut snap = Snapshot::new(1);
        snap.add_component(100, ComponentData::new(1, vec![10, 20]));
        snap.add_component(100, ComponentData::new(2, vec![30, 40]));
        snap.add_component(200, ComponentData::new(1, vec![50]));

        assert_eq!(snap.entity_count(), 2);
        assert_eq!(snap.component_count(), 3);
    }

    #[test]
    fn test_snapshot_encode_decode_roundtrip() {
        let mut snap = Snapshot::new(42);
        snap.add_component(1, ComponentData::new(10, vec![1, 2, 3]));
        snap.add_component(1, ComponentData::new(20, vec![4, 5]));
        snap.add_component(2, ComponentData::new(10, vec![6, 7, 8, 9]));
        snap.add_component(3, ComponentData::new(30, vec![]));

        let encoded = snap.encode();
        let decoded = Snapshot::decode(&encoded).unwrap();

        assert_eq!(decoded.tick, snap.tick);
        assert_eq!(decoded.entity_count(), snap.entity_count());
        assert_eq!(decoded.component_count(), snap.component_count());
        assert_eq!(decoded, snap);
    }

    #[test]
    fn test_snapshot_decode_too_short() {
        assert!(Snapshot::decode(&[0; 4]).is_none());
    }

    // -- Delta computation tests ----------------------------------------------

    fn make_test_snapshots() -> (Snapshot, Snapshot) {
        let mut base = Snapshot::new(1);
        // Entity 1: position (type 1) + velocity (type 2)
        base.add_component(1, ComponentData::new(1, vec![0, 0, 0, 0])); // x=0
        base.add_component(1, ComponentData::new(2, vec![1, 0, 0, 0])); // vx=1
        // Entity 2: position only
        base.add_component(2, ComponentData::new(1, vec![10, 0, 0, 0]));
        // Entity 3: will be removed
        base.add_component(3, ComponentData::new(1, vec![20, 0, 0, 0]));

        let mut target = Snapshot::new(2);
        // Entity 1: position changed, velocity same
        target.add_component(1, ComponentData::new(1, vec![1, 0, 0, 0])); // x=1 (changed)
        target.add_component(1, ComponentData::new(2, vec![1, 0, 0, 0])); // vx=1 (same)
        // Entity 2: unchanged
        target.add_component(2, ComponentData::new(1, vec![10, 0, 0, 0]));
        // Entity 3: removed (not present)
        // Entity 4: new
        target.add_component(4, ComponentData::new(1, vec![30, 0, 0, 0]));

        (base, target)
    }

    #[test]
    fn test_delta_compute() {
        let (base, target) = make_test_snapshots();
        let delta = DeltaCompression::compute(&base, &target);

        assert_eq!(delta.base_tick, 1);
        assert_eq!(delta.target_tick, 2);
        // Entity 3 should be removed
        assert!(delta.removed_entities.contains(&3));
        // Entity 4 is new
        assert!(delta.entity_deltas.iter().any(|ed| ed.entity_id == 4));
        // Entity 1 has one changed component (position)
        let e1_delta = delta.entity_deltas.iter().find(|ed| ed.entity_id == 1);
        assert!(e1_delta.is_some());
        let e1_changes = &e1_delta.unwrap().changes;
        assert_eq!(e1_changes.len(), 1); // only position changed
        // Entity 2 is unchanged, should not appear in deltas
        assert!(!delta.entity_deltas.iter().any(|ed| ed.entity_id == 2));
    }

    #[test]
    fn test_delta_apply_equals_target() {
        let (base, target) = make_test_snapshots();
        let delta = DeltaCompression::compute(&base, &target);
        let reconstructed = DeltaCompression::apply(&base, &delta);

        assert_eq!(reconstructed.tick, target.tick);
        assert_eq!(reconstructed.entity_count(), target.entity_count());
        assert_eq!(reconstructed, target);
    }

    #[test]
    fn test_delta_empty_when_identical() {
        let mut snap = Snapshot::new(1);
        snap.add_component(1, ComponentData::new(1, vec![1, 2, 3]));

        let delta = DeltaCompression::compute(&snap, &snap);
        assert!(delta.is_empty());
    }

    #[test]
    fn test_delta_compression_ratio() {
        // Build a large snapshot where only 2 out of 20 entities change
        let mut base = Snapshot::new(1);
        for eid in 0..20 {
            base.add_component(eid, ComponentData::new(1, vec![0u8; 32]));
            base.add_component(eid, ComponentData::new(2, vec![0u8; 32]));
        }

        let mut target = base.clone();
        target.tick = 2;
        // Change 2 entities
        target.entities.get_mut(&5).unwrap()[0].data = vec![1u8; 32];
        target.entities.get_mut(&15).unwrap()[1].data = vec![2u8; 32];

        let delta = DeltaCompression::compute(&base, &target);
        let delta_encoded = delta.encode();
        let snapshot_encoded = target.encode();

        // Delta should be significantly smaller than full snapshot
        assert!(
            delta_encoded.len() * 2 <= snapshot_encoded.len(),
            "delta {} bytes should be <= 50% of full snapshot {} bytes",
            delta_encoded.len(),
            snapshot_encoded.len()
        );
    }

    #[test]
    fn test_delta_encode_decode_roundtrip() {
        let (base, target) = make_test_snapshots();
        let delta = DeltaCompression::compute(&base, &target);

        let encoded = delta.encode();
        let decoded = Delta::decode(&encoded).unwrap();

        assert_eq!(decoded.base_tick, delta.base_tick);
        assert_eq!(decoded.target_tick, delta.target_tick);
        assert_eq!(decoded.removed_entities, delta.removed_entities);
        assert_eq!(decoded.entity_deltas.len(), delta.entity_deltas.len());

        // Apply decoded delta and verify it produces the same result
        let from_decoded = DeltaCompression::apply(&base, &decoded);
        let from_original = DeltaCompression::apply(&base, &delta);
        assert_eq!(from_decoded, from_original);
    }

    #[test]
    fn test_delta_component_removed() {
        let mut base = Snapshot::new(1);
        base.add_component(1, ComponentData::new(1, vec![1, 2]));
        base.add_component(1, ComponentData::new(2, vec![3, 4]));

        let mut target = Snapshot::new(2);
        target.add_component(1, ComponentData::new(1, vec![1, 2])); // keep type 1
        // type 2 removed

        let delta = DeltaCompression::compute(&base, &target);
        let e1 = delta.entity_deltas.iter().find(|ed| ed.entity_id == 1).unwrap();
        assert!(e1.changes.contains(&FieldChange::Removed(2)));

        let reconstructed = DeltaCompression::apply(&base, &delta);
        assert_eq!(reconstructed, target);
    }

    #[test]
    fn test_delta_new_entity_with_multiple_components() {
        let base = Snapshot::new(1);
        let mut target = Snapshot::new(2);
        target.add_component(10, ComponentData::new(1, vec![10]));
        target.add_component(10, ComponentData::new(2, vec![20]));
        target.add_component(10, ComponentData::new(3, vec![30]));

        let delta = DeltaCompression::compute(&base, &target);
        let reconstructed = DeltaCompression::apply(&base, &delta);
        assert_eq!(reconstructed, target);
    }

    // -- NetworkedComponent trait test (example impl) -------------------------

    #[derive(Debug, Clone, PartialEq)]
    struct TestPosition {
        x: f32,
        y: f32,
    }

    impl NetworkedComponent for TestPosition {
        const TYPE_ID: ComponentTypeId = 1;

        fn net_serialize(&self) -> Vec<u8> {
            let mut buf = Vec::with_capacity(8);
            buf.extend_from_slice(&self.x.to_le_bytes());
            buf.extend_from_slice(&self.y.to_le_bytes());
            buf
        }

        fn net_deserialize(data: &[u8]) -> Option<Self> {
            if data.len() < 8 {
                return None;
            }
            let x = f32::from_le_bytes(data[0..4].try_into().ok()?);
            let y = f32::from_le_bytes(data[4..8].try_into().ok()?);
            Some(Self { x, y })
        }
    }

    #[test]
    fn test_networked_component_roundtrip() {
        let pos = TestPosition { x: 1.5, y: -3.7 };
        let serialized = pos.net_serialize();
        let deserialized = TestPosition::net_deserialize(&serialized).unwrap();
        assert_eq!(deserialized, pos);
    }

    #[test]
    fn test_networked_component_as_component_data() {
        let pos = TestPosition { x: 42.0, y: 99.0 };
        let comp = ComponentData::new(TestPosition::TYPE_ID, pos.net_serialize());

        // Encode -> decode -> deserialize
        let encoded = comp.encode();
        let (decoded_comp, _) = ComponentData::decode(&encoded).unwrap();
        let decoded_pos = TestPosition::net_deserialize(&decoded_comp.data).unwrap();
        assert_eq!(decoded_pos, pos);
    }
}
