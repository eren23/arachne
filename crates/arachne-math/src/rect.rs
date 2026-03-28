use crate::vec2::Vec2;

/// An axis-aligned bounding rectangle defined by its minimum and maximum corners.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub min: Vec2,
    pub max: Vec2,
}

impl Rect {
    /// Creates a new [`Rect`] from minimum and maximum corners.
    ///
    /// The caller is responsible for ensuring `min <= max` component-wise.
    #[inline]
    pub fn new(min: Vec2, max: Vec2) -> Self {
        Self { min, max }
    }

    /// Creates a new [`Rect`] from a center point and a full size (width, height).
    #[inline]
    pub fn from_center_size(center: Vec2, size: Vec2) -> Self {
        let half = size * 0.5;
        Self {
            min: center - half,
            max: center + half,
        }
    }

    /// Creates a new [`Rect`] from a minimum corner and a size (width, height).
    #[inline]
    pub fn from_min_size(min: Vec2, size: Vec2) -> Self {
        Self {
            min,
            max: min + size,
        }
    }

    /// Returns the width of the rectangle.
    #[inline]
    pub fn width(self) -> f32 {
        self.max.x - self.min.x
    }

    /// Returns the height of the rectangle.
    #[inline]
    pub fn height(self) -> f32 {
        self.max.y - self.min.y
    }

    /// Returns the size of the rectangle as a [`Vec2`] `(width, height)`.
    #[inline]
    pub fn size(self) -> Vec2 {
        self.max - self.min
    }

    /// Returns the center point of the rectangle.
    #[inline]
    pub fn center(self) -> Vec2 {
        (self.min + self.max) * 0.5
    }

    /// Returns `true` if the rectangle contains the given point.
    ///
    /// The check is inclusive on all edges (min and max).
    #[inline]
    pub fn contains(self, point: Vec2) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
    }

    /// Returns `true` if `self` and `other` overlap.
    ///
    /// Two rectangles that merely touch at an edge or corner are **not**
    /// considered to be intersecting.
    #[inline]
    pub fn intersects(self, other: Rect) -> bool {
        self.min.x < other.max.x
            && self.max.x > other.min.x
            && self.min.y < other.max.y
            && self.max.y > other.min.y
    }

    /// Returns the intersection of `self` and `other`, or `None` if they do not
    /// overlap.
    #[inline]
    pub fn intersection(self, other: Rect) -> Option<Rect> {
        let min = self.min.max(other.min);
        let max = self.max.min(other.max);
        if min.x < max.x && min.y < max.y {
            Some(Rect { min, max })
        } else {
            None
        }
    }

    /// Returns the smallest rectangle that contains both `self` and `other`.
    #[inline]
    pub fn union(self, other: Rect) -> Rect {
        Rect {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }

    /// Returns a new rectangle expanded on all sides by `amount`.
    ///
    /// A negative `amount` shrinks the rectangle.
    #[inline]
    pub fn expand(self, amount: f32) -> Rect {
        let offset = Vec2::splat(amount);
        Rect {
            min: self.min - offset,
            max: self.max + offset,
        }
    }

    /// Returns `true` if the rectangle has zero or negative area in any
    /// dimension (i.e. `min >= max` on either axis).
    #[inline]
    pub fn is_empty(self) -> bool {
        self.min.x >= self.max.x || self.min.y >= self.max.y
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- contains --

    #[test]
    fn contains_point_inside() {
        let r = Rect::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0));
        assert!(r.contains(Vec2::new(5.0, 5.0)));
    }

    #[test]
    fn contains_point_on_min_edge() {
        let r = Rect::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0));
        assert!(r.contains(Vec2::new(0.0, 0.0)));
        assert!(r.contains(Vec2::new(0.0, 5.0)));
        assert!(r.contains(Vec2::new(5.0, 0.0)));
    }

    #[test]
    fn contains_point_on_max_edge() {
        let r = Rect::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0));
        assert!(r.contains(Vec2::new(10.0, 10.0)));
        assert!(r.contains(Vec2::new(10.0, 5.0)));
        assert!(r.contains(Vec2::new(5.0, 10.0)));
    }

    #[test]
    fn contains_point_outside() {
        let r = Rect::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0));
        assert!(!r.contains(Vec2::new(-1.0, 5.0)));
        assert!(!r.contains(Vec2::new(5.0, -1.0)));
        assert!(!r.contains(Vec2::new(11.0, 5.0)));
        assert!(!r.contains(Vec2::new(5.0, 11.0)));
    }

    // -- intersects --

    #[test]
    fn intersects_overlapping() {
        let a = Rect::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0));
        let b = Rect::new(Vec2::new(5.0, 5.0), Vec2::new(15.0, 15.0));
        assert!(a.intersects(b));
        assert!(b.intersects(a));
    }

    #[test]
    fn intersects_non_overlapping() {
        let a = Rect::new(Vec2::new(0.0, 0.0), Vec2::new(5.0, 5.0));
        let b = Rect::new(Vec2::new(10.0, 10.0), Vec2::new(20.0, 20.0));
        assert!(!a.intersects(b));
        assert!(!b.intersects(a));
    }

    #[test]
    fn intersects_touching_edges_is_false() {
        let a = Rect::new(Vec2::new(0.0, 0.0), Vec2::new(5.0, 5.0));
        let b = Rect::new(Vec2::new(5.0, 0.0), Vec2::new(10.0, 5.0));
        assert!(!a.intersects(b));
        assert!(!b.intersects(a));
    }

    // -- intersection --

    #[test]
    fn intersection_overlapping_returns_correct_rect() {
        let a = Rect::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0));
        let b = Rect::new(Vec2::new(5.0, 5.0), Vec2::new(15.0, 15.0));
        let i = a.intersection(b).expect("should intersect");
        assert_eq!(i.min, Vec2::new(5.0, 5.0));
        assert_eq!(i.max, Vec2::new(10.0, 10.0));
    }

    #[test]
    fn intersection_non_overlapping_returns_none() {
        let a = Rect::new(Vec2::new(0.0, 0.0), Vec2::new(5.0, 5.0));
        let b = Rect::new(Vec2::new(10.0, 10.0), Vec2::new(20.0, 20.0));
        assert!(a.intersection(b).is_none());
    }

    #[test]
    fn intersection_touching_edge_returns_none() {
        let a = Rect::new(Vec2::new(0.0, 0.0), Vec2::new(5.0, 5.0));
        let b = Rect::new(Vec2::new(5.0, 0.0), Vec2::new(10.0, 5.0));
        assert!(a.intersection(b).is_none());
    }

    // -- union --

    #[test]
    fn union_covers_both_rects() {
        let a = Rect::new(Vec2::new(0.0, 0.0), Vec2::new(5.0, 5.0));
        let b = Rect::new(Vec2::new(3.0, 3.0), Vec2::new(10.0, 10.0));
        let u = a.union(b);
        assert_eq!(u.min, Vec2::new(0.0, 0.0));
        assert_eq!(u.max, Vec2::new(10.0, 10.0));
    }

    #[test]
    fn union_disjoint_rects() {
        let a = Rect::new(Vec2::new(-5.0, -5.0), Vec2::new(-1.0, -1.0));
        let b = Rect::new(Vec2::new(1.0, 1.0), Vec2::new(5.0, 5.0));
        let u = a.union(b);
        assert_eq!(u.min, Vec2::new(-5.0, -5.0));
        assert_eq!(u.max, Vec2::new(5.0, 5.0));
    }

    // -- expand --

    #[test]
    fn expand_positive() {
        let r = Rect::new(Vec2::new(2.0, 3.0), Vec2::new(8.0, 7.0));
        let expanded = r.expand(1.0);
        assert_eq!(expanded.min, Vec2::new(1.0, 2.0));
        assert_eq!(expanded.max, Vec2::new(9.0, 8.0));
    }

    #[test]
    fn expand_negative_shrinks() {
        let r = Rect::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0));
        let shrunk = r.expand(-2.0);
        assert_eq!(shrunk.min, Vec2::new(2.0, 2.0));
        assert_eq!(shrunk.max, Vec2::new(8.0, 8.0));
    }

    // -- from_center_size roundtrip --

    #[test]
    fn from_center_size_roundtrip() {
        let center = Vec2::new(5.0, 10.0);
        let size = Vec2::new(6.0, 8.0);
        let r = Rect::from_center_size(center, size);
        assert_eq!(r.center(), center);
        assert_eq!(r.size(), size);
    }

    #[test]
    fn from_center_size_min_max() {
        let r = Rect::from_center_size(Vec2::new(5.0, 5.0), Vec2::new(4.0, 6.0));
        assert_eq!(r.min, Vec2::new(3.0, 2.0));
        assert_eq!(r.max, Vec2::new(7.0, 8.0));
    }

    // -- from_min_size --

    #[test]
    fn from_min_size_basic() {
        let r = Rect::from_min_size(Vec2::new(1.0, 2.0), Vec2::new(3.0, 4.0));
        assert_eq!(r.min, Vec2::new(1.0, 2.0));
        assert_eq!(r.max, Vec2::new(4.0, 6.0));
        assert_eq!(r.width(), 3.0);
        assert_eq!(r.height(), 4.0);
    }

    // -- is_empty --

    #[test]
    fn is_empty_zero_width() {
        let r = Rect::new(Vec2::new(5.0, 0.0), Vec2::new(5.0, 10.0));
        assert!(r.is_empty());
    }

    #[test]
    fn is_empty_zero_height() {
        let r = Rect::new(Vec2::new(0.0, 5.0), Vec2::new(10.0, 5.0));
        assert!(r.is_empty());
    }

    #[test]
    fn is_empty_inverted() {
        let r = Rect::new(Vec2::new(10.0, 10.0), Vec2::new(0.0, 0.0));
        assert!(r.is_empty());
    }

    #[test]
    fn is_empty_valid_rect() {
        let r = Rect::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0));
        assert!(!r.is_empty());
    }

    // -- width / height / size / center --

    #[test]
    fn width_height_size() {
        let r = Rect::new(Vec2::new(1.0, 2.0), Vec2::new(4.0, 7.0));
        assert_eq!(r.width(), 3.0);
        assert_eq!(r.height(), 5.0);
        assert_eq!(r.size(), Vec2::new(3.0, 5.0));
    }

    #[test]
    fn center_basic() {
        let r = Rect::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 20.0));
        assert_eq!(r.center(), Vec2::new(5.0, 10.0));
    }
}
