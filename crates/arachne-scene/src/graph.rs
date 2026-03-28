use arachne_ecs::Entity;

// COMPONENTS ------

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Parent(pub Entity);

#[derive(Clone, Debug, PartialEq)]
pub struct Children(pub Vec<Entity>);

// SCENE_GRAPH ------

pub struct SceneGraph {
    parents: Vec<Option<Entity>>,
    children: Vec<Vec<Entity>>,
    exists: Vec<bool>,
    count: usize,
}

impl SceneGraph {
    #[inline]
    pub fn new() -> Self {
        Self {
            parents: Vec::new(),
            children: Vec::new(),
            exists: Vec::new(),
            count: 0,
        }
    }

    fn ensure_capacity(&mut self, index: usize) {
        if index >= self.parents.len() {
            let new_len = index + 1;
            self.parents.resize(new_len, None);
            self.children.resize_with(new_len, Vec::new);
            self.exists.resize(new_len, false);
        }
    }

    fn track(&mut self, entity: Entity) {
        let idx = entity.index() as usize;
        self.ensure_capacity(idx);
        if !self.exists[idx] {
            self.exists[idx] = true;
            self.count += 1;
        }
    }

    #[inline]
    pub fn parent_of(&self, entity: Entity) -> Option<Entity> {
        let idx = entity.index() as usize;
        if idx < self.parents.len() {
            self.parents[idx]
        } else {
            None
        }
    }

    #[inline]
    pub fn children_of(&self, entity: Entity) -> &[Entity] {
        let idx = entity.index() as usize;
        if idx < self.children.len() {
            &self.children[idx]
        } else {
            &[]
        }
    }

    pub fn roots(&self) -> Vec<Entity> {
        let mut roots = Vec::new();
        for (i, &exists) in self.exists.iter().enumerate() {
            if exists && self.parents[i].is_none() {
                roots.push(Entity::from_raw(i as u32, 0));
            }
        }
        roots
    }

    pub fn set_parent(&mut self, child: Entity, parent: Entity) {
        self.track(child);
        self.track(parent);

        let child_idx = child.index() as usize;

        // Remove from old parent if any
        if let Some(old_parent) = self.parents[child_idx] {
            let old_idx = old_parent.index() as usize;
            self.children[old_idx].retain(|&e| e != child);
        }

        self.parents[child_idx] = Some(parent);
        let parent_idx = parent.index() as usize;
        self.children[parent_idx].push(child);
    }

    #[inline]
    pub fn add_child(&mut self, parent: Entity, child: Entity) {
        self.set_parent(child, parent);
    }

    pub fn remove_child(&mut self, parent: Entity, child: Entity) {
        let parent_idx = parent.index() as usize;
        if parent_idx < self.children.len() {
            self.children[parent_idx].retain(|&e| e != child);
        }
        let child_idx = child.index() as usize;
        if child_idx < self.parents.len() && self.parents[child_idx] == Some(parent) {
            self.parents[child_idx] = None;
        }
    }

    pub fn remove_parent(&mut self, entity: Entity) {
        let idx = entity.index() as usize;
        if idx >= self.parents.len() {
            return;
        }
        if let Some(old_parent) = self.parents[idx].take() {
            let old_idx = old_parent.index() as usize;
            if old_idx < self.children.len() {
                self.children[old_idx].retain(|&e| e != entity);
            }
        }
    }

    pub fn remove_entity(&mut self, entity: Entity) {
        let idx = entity.index() as usize;
        if idx >= self.exists.len() || !self.exists[idx] {
            return;
        }

        // Remove as child from parent
        self.remove_parent(entity);

        // Orphan all children
        let child_list = std::mem::take(&mut self.children[idx]);
        for child in child_list {
            let ci = child.index() as usize;
            if ci < self.parents.len() {
                self.parents[ci] = None;
            }
        }

        self.exists[idx] = false;
        self.count -= 1;
    }

    pub fn dfs_iter(&self) -> DfsIter<'_> {
        let roots = self.roots();
        let mut stack = Vec::new();
        for &root in roots.iter().rev() {
            stack.push(root);
        }
        DfsIter {
            graph: self,
            stack,
        }
    }

    #[inline]
    pub fn contains(&self, entity: Entity) -> bool {
        let idx = entity.index() as usize;
        idx < self.exists.len() && self.exists[idx]
    }

    #[inline]
    pub fn entity_count(&self) -> usize {
        self.count
    }
}

impl Default for SceneGraph {
    fn default() -> Self {
        Self::new()
    }
}

// DFS_ITER ------

pub struct DfsIter<'a> {
    graph: &'a SceneGraph,
    stack: Vec<Entity>,
}

impl<'a> Iterator for DfsIter<'a> {
    type Item = Entity;

    fn next(&mut self) -> Option<Entity> {
        let entity = self.stack.pop()?;
        let children = self.graph.children_of(entity);
        for &child in children.iter().rev() {
            self.stack.push(child);
        }
        Some(entity)
    }
}

// TESTS ------

#[cfg(test)]
mod tests {
    use super::*;

    fn e(index: u32) -> Entity {
        Entity::from_raw(index, 0)
    }

    #[test]
    fn hierarchy_parent_three_children() {
        let mut graph = SceneGraph::new();
        let parent = e(0);
        let c1 = e(1);
        let c2 = e(2);
        let c3 = e(3);

        graph.add_child(parent, c1);
        graph.add_child(parent, c2);
        graph.add_child(parent, c3);

        assert_eq!(graph.parent_of(c1), Some(parent));
        assert_eq!(graph.parent_of(c2), Some(parent));
        assert_eq!(graph.parent_of(c3), Some(parent));
        assert_eq!(graph.children_of(parent), &[c1, c2, c3]);
        assert_eq!(graph.parent_of(parent), None);

        let roots = graph.roots();
        assert_eq!(roots, vec![parent]);
    }

    #[test]
    fn reparent_child() {
        let mut graph = SceneGraph::new();
        let p1 = e(0);
        let p2 = e(1);
        let child = e(2);

        graph.add_child(p1, child);
        assert_eq!(graph.parent_of(child), Some(p1));
        assert_eq!(graph.children_of(p1), &[child]);

        graph.set_parent(child, p2);
        assert_eq!(graph.parent_of(child), Some(p2));
        assert_eq!(graph.children_of(p2), &[child]);
        assert!(graph.children_of(p1).is_empty());
    }

    #[test]
    fn remove_parent_makes_root() {
        let mut graph = SceneGraph::new();
        let parent = e(0);
        let child = e(1);

        graph.add_child(parent, child);
        assert_eq!(graph.parent_of(child), Some(parent));

        graph.remove_parent(child);
        assert_eq!(graph.parent_of(child), None);
        assert!(graph.children_of(parent).is_empty());

        let roots = graph.roots();
        assert!(roots.contains(&parent));
        assert!(roots.contains(&child));
    }

    #[test]
    fn dfs_iteration_order() {
        //       0
        //      / \
        //     1   2
        //    / \
        //   3   4
        let mut graph = SceneGraph::new();
        let n0 = e(0);
        let n1 = e(1);
        let n2 = e(2);
        let n3 = e(3);
        let n4 = e(4);

        graph.add_child(n0, n1);
        graph.add_child(n0, n2);
        graph.add_child(n1, n3);
        graph.add_child(n1, n4);

        let order: Vec<Entity> = graph.dfs_iter().collect();
        assert_eq!(order, vec![n0, n1, n3, n4, n2]);
    }

    #[test]
    fn remove_entity_orphans_children() {
        let mut graph = SceneGraph::new();
        let root = e(0);
        let mid = e(1);
        let leaf = e(2);

        graph.add_child(root, mid);
        graph.add_child(mid, leaf);

        graph.remove_entity(mid);
        assert_eq!(graph.parent_of(mid), None);
        assert_eq!(graph.parent_of(leaf), None);
        assert!(graph.children_of(root).is_empty());
    }
}
