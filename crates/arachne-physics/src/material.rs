/// Physical material properties for a collider.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicsMaterial {
    /// Coulomb friction coefficient.
    pub friction: f32,
    /// Coefficient of restitution: 0 = perfectly inelastic, 1 = perfectly elastic.
    pub restitution: f32,
    /// Density in kg/m^2 (used for automatic mass calculation).
    pub density: f32,
}

impl Default for PhysicsMaterial {
    fn default() -> Self {
        Self {
            friction: 0.3,
            restitution: 0.0,
            density: 1.0,
        }
    }
}

impl PhysicsMaterial {
    pub fn new(friction: f32, restitution: f32) -> Self {
        Self {
            friction,
            restitution,
            density: 1.0,
        }
    }

    pub fn with_density(mut self, density: f32) -> Self {
        self.density = density;
        self
    }

    /// Combines friction using geometric mean: sqrt(a * b).
    #[inline]
    pub fn combine_friction(a: f32, b: f32) -> f32 {
        (a * b).sqrt()
    }

    /// Combines restitution using max(a, b).
    #[inline]
    pub fn combine_restitution(a: f32, b: f32) -> f32 {
        a.max(b)
    }

    /// Combine two materials into one effective material for a contact pair.
    pub fn combine(a: &PhysicsMaterial, b: &PhysicsMaterial) -> CombinedMaterial {
        CombinedMaterial {
            friction: Self::combine_friction(a.friction, b.friction),
            restitution: Self::combine_restitution(a.restitution, b.restitution),
        }
    }
}

/// The combined material properties for a contact pair.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CombinedMaterial {
    pub friction: f32,
    pub restitution: f32,
}

// ---------------------------------------------------------------------------
// Material presets
// ---------------------------------------------------------------------------

/// Common material presets for quick setup.
pub struct MaterialPreset;

impl MaterialPreset {
    /// Rubber: high friction, high restitution.
    pub fn rubber() -> PhysicsMaterial {
        PhysicsMaterial {
            friction: 0.8,
            restitution: 0.9,
            density: 1.2,
        }
    }

    /// Ice: very low friction, no bounce.
    pub fn ice() -> PhysicsMaterial {
        PhysicsMaterial {
            friction: 0.02,
            restitution: 0.0,
            density: 0.9,
        }
    }

    /// Metal: medium friction, low restitution.
    pub fn metal() -> PhysicsMaterial {
        PhysicsMaterial {
            friction: 0.4,
            restitution: 0.1,
            density: 7.8,
        }
    }

    /// Wood: moderate friction, low restitution.
    pub fn wood() -> PhysicsMaterial {
        PhysicsMaterial {
            friction: 0.5,
            restitution: 0.15,
            density: 0.6,
        }
    }

    /// Bouncy ball: moderate friction, near-perfect restitution.
    pub fn bouncy() -> PhysicsMaterial {
        PhysicsMaterial {
            friction: 0.4,
            restitution: 0.95,
            density: 1.0,
        }
    }

    /// Glass: low friction, moderate restitution.
    pub fn glass() -> PhysicsMaterial {
        PhysicsMaterial {
            friction: 0.1,
            restitution: 0.3,
            density: 2.5,
        }
    }

    /// Concrete: high friction, no bounce.
    pub fn concrete() -> PhysicsMaterial {
        PhysicsMaterial {
            friction: 0.7,
            restitution: 0.0,
            density: 2.4,
        }
    }

    /// Mud: very high friction, no bounce.
    pub fn mud() -> PhysicsMaterial {
        PhysicsMaterial {
            friction: 0.95,
            restitution: 0.0,
            density: 1.5,
        }
    }
}

// ---------------------------------------------------------------------------
// MaterialTable – shared material definitions
// ---------------------------------------------------------------------------

/// A lookup table of named materials. Bodies reference materials by index.
#[derive(Clone, Debug)]
pub struct MaterialTable {
    entries: Vec<(String, PhysicsMaterial)>,
}

impl MaterialTable {
    pub fn new() -> Self {
        let mut table = Self {
            entries: Vec::new(),
        };
        table.add("default", PhysicsMaterial::default());
        table
    }

    /// Add a named material. Returns its index.
    pub fn add(&mut self, name: &str, material: PhysicsMaterial) -> usize {
        let idx = self.entries.len();
        self.entries.push((name.to_string(), material));
        idx
    }

    /// Look up a material by index.
    pub fn get(&self, index: usize) -> &PhysicsMaterial {
        &self.entries[index].1
    }

    /// Look up a material by name.
    pub fn find(&self, name: &str) -> Option<(usize, &PhysicsMaterial)> {
        self.entries
            .iter()
            .enumerate()
            .find(|(_, (n, _))| n == name)
            .map(|(i, (_, m))| (i, m))
    }

    /// Number of materials in the table.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Combine two materials by index.
    pub fn combine(&self, a: usize, b: usize) -> CombinedMaterial {
        PhysicsMaterial::combine(self.get(a), self.get(b))
    }
}

impl Default for MaterialTable {
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
    fn default_material() {
        let m = PhysicsMaterial::default();
        assert!((m.friction - 0.3).abs() < 1e-6);
        assert!((m.restitution - 0.0).abs() < 1e-6);
        assert!((m.density - 1.0).abs() < 1e-6);
    }

    #[test]
    fn combine_friction_geometric_mean() {
        let f = PhysicsMaterial::combine_friction(0.25, 0.64);
        assert!((f - 0.4).abs() < 1e-6);
    }

    #[test]
    fn combine_friction_zero() {
        assert!((PhysicsMaterial::combine_friction(0.0, 0.5) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn combine_friction_same() {
        let f = PhysicsMaterial::combine_friction(0.5, 0.5);
        assert!((f - 0.5).abs() < 1e-6);
    }

    #[test]
    fn combine_restitution_max() {
        assert!((PhysicsMaterial::combine_restitution(0.3, 0.7) - 0.7).abs() < 1e-6);
        assert!((PhysicsMaterial::combine_restitution(0.9, 0.1) - 0.9).abs() < 1e-6);
    }

    #[test]
    fn combine_two_materials() {
        let a = MaterialPreset::rubber();
        let b = MaterialPreset::ice();
        let combined = PhysicsMaterial::combine(&a, &b);
        // friction: sqrt(0.8 * 0.02) ≈ 0.1265
        assert!((combined.friction - (0.8f32 * 0.02).sqrt()).abs() < 1e-4);
        // restitution: max(0.9, 0.0) = 0.9
        assert!((combined.restitution - 0.9).abs() < 1e-6);
    }

    #[test]
    fn material_with_density() {
        let m = PhysicsMaterial::new(0.5, 0.3).with_density(2.0);
        assert_eq!(m.density, 2.0);
    }

    // -- Preset tests -----------------------------------------------------

    #[test]
    fn preset_rubber() {
        let m = MaterialPreset::rubber();
        assert!(m.friction > 0.5);
        assert!(m.restitution > 0.8);
    }

    #[test]
    fn preset_ice() {
        let m = MaterialPreset::ice();
        assert!(m.friction < 0.1);
        assert!(m.restitution < 0.1);
    }

    #[test]
    fn preset_bouncy() {
        let m = MaterialPreset::bouncy();
        assert!(m.restitution > 0.9);
    }

    #[test]
    fn preset_metal() {
        let m = MaterialPreset::metal();
        assert!(m.density > 5.0);
    }

    #[test]
    fn all_presets_valid() {
        let presets = [
            MaterialPreset::rubber(),
            MaterialPreset::ice(),
            MaterialPreset::metal(),
            MaterialPreset::wood(),
            MaterialPreset::bouncy(),
            MaterialPreset::glass(),
            MaterialPreset::concrete(),
            MaterialPreset::mud(),
        ];
        for p in &presets {
            assert!(p.friction >= 0.0 && p.friction <= 1.0);
            assert!(p.restitution >= 0.0 && p.restitution <= 1.0);
            assert!(p.density > 0.0);
        }
    }

    // -- MaterialTable tests ----------------------------------------------

    #[test]
    fn material_table_default_has_default() {
        let table = MaterialTable::new();
        assert_eq!(table.len(), 1);
        let (idx, mat) = table.find("default").unwrap();
        assert_eq!(idx, 0);
        assert!((mat.friction - 0.3).abs() < 1e-6);
    }

    #[test]
    fn material_table_add_and_get() {
        let mut table = MaterialTable::new();
        let idx = table.add("rubber", MaterialPreset::rubber());
        let mat = table.get(idx);
        assert!((mat.friction - 0.8).abs() < 1e-6);
    }

    #[test]
    fn material_table_find_by_name() {
        let mut table = MaterialTable::new();
        table.add("ice", MaterialPreset::ice());
        let result = table.find("ice");
        assert!(result.is_some());
        let (_, mat) = result.unwrap();
        assert!(mat.friction < 0.1);
    }

    #[test]
    fn material_table_find_missing() {
        let table = MaterialTable::new();
        assert!(table.find("nonexistent").is_none());
    }

    #[test]
    fn material_table_combine() {
        let mut table = MaterialTable::new();
        let rubber = table.add("rubber", MaterialPreset::rubber());
        let ice = table.add("ice", MaterialPreset::ice());

        let combined = table.combine(rubber, ice);
        assert!(combined.friction > 0.0);
        assert!((combined.restitution - 0.9).abs() < 1e-6);
    }
}
