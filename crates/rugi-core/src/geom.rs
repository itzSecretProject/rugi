//! Minimal 2D geometry in logical (DPI-independent) pixels.

use std::ops::{Add, AddAssign, Mul, Sub};

/// A 2D vector / point in logical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    /// Horizontal component.
    pub x: f32,
    /// Vertical component (grows downwards).
    pub y: f32,
}

impl Vec2 {
    /// The zero vector.
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };

    /// Creates a vector.
    #[inline]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Component-wise maximum.
    #[inline]
    pub fn max(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x.max(o.x), self.y.max(o.y))
    }

    /// Component-wise minimum.
    #[inline]
    pub fn min(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x.min(o.x), self.y.min(o.y))
    }
}

impl Add for Vec2 {
    type Output = Vec2;
    #[inline]
    fn add(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x + o.x, self.y + o.y)
    }
}

impl AddAssign for Vec2 {
    #[inline]
    fn add_assign(&mut self, o: Vec2) {
        self.x += o.x;
        self.y += o.y;
    }
}

impl Sub for Vec2 {
    type Output = Vec2;
    #[inline]
    fn sub(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x - o.x, self.y - o.y)
    }
}

impl Mul<f32> for Vec2 {
    type Output = Vec2;
    #[inline]
    fn mul(self, s: f32) -> Vec2 {
        Vec2::new(self.x * s, self.y * s)
    }
}

/// An axis-aligned rectangle defined by its min (top-left) and max
/// (bottom-right) corners, in logical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    /// Top-left corner.
    pub min: Vec2,
    /// Bottom-right corner.
    pub max: Vec2,
}

impl Rect {
    /// An empty rectangle at the origin.
    pub const ZERO: Rect = Rect { min: Vec2::ZERO, max: Vec2::ZERO };

    /// A rectangle covering (practically) everything; used as "no clip".
    pub const EVERYTHING: Rect = Rect { min: Vec2 { x: -1.0e7, y: -1.0e7 }, max: Vec2 { x: 1.0e7, y: 1.0e7 } };

    /// Creates a rectangle from position and size.
    #[inline]
    pub fn from_xywh(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { min: Vec2::new(x, y), max: Vec2::new(x + w, y + h) }
    }

    /// Creates a rectangle from min/max corners.
    #[inline]
    pub fn from_min_max(min: Vec2, max: Vec2) -> Self {
        Self { min, max }
    }

    /// Creates a rectangle from its min corner and a size.
    #[inline]
    pub fn from_min_size(min: Vec2, size: Vec2) -> Self {
        Self { min, max: min + size }
    }

    /// A rectangle of `size` centered on `center`.
    #[inline]
    pub fn from_center_size(center: Vec2, size: Vec2) -> Self {
        Self::from_min_size(Vec2::new(center.x - size.x * 0.5, center.y - size.y * 0.5), size)
    }

    /// Width.
    #[inline]
    pub fn width(&self) -> f32 {
        self.max.x - self.min.x
    }

    /// Height.
    #[inline]
    pub fn height(&self) -> f32 {
        self.max.y - self.min.y
    }

    /// Size as a vector.
    #[inline]
    pub fn size(&self) -> Vec2 {
        self.max - self.min
    }

    /// Center point.
    #[inline]
    pub fn center(&self) -> Vec2 {
        Vec2::new((self.min.x + self.max.x) * 0.5, (self.min.y + self.max.y) * 0.5)
    }

    /// True if the point lies inside (min inclusive, max exclusive).
    #[inline]
    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.min.x && p.x < self.max.x && p.y >= self.min.y && p.y < self.max.y
    }

    /// Intersection of two rectangles (may be empty / inverted, check with [`Rect::is_empty`]).
    #[inline]
    pub fn intersect(&self, o: &Rect) -> Rect {
        Rect { min: self.min.max(o.min), max: self.max.min(o.max) }
    }

    /// True if the rectangles overlap with positive area.
    #[inline]
    pub fn intersects(&self, o: &Rect) -> bool {
        self.min.x < o.max.x && o.min.x < self.max.x && self.min.y < o.max.y && o.min.y < self.max.y
    }

    /// True if width or height is not positive.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.max.x <= self.min.x || self.max.y <= self.min.y
    }

    /// Grows the rectangle by `amount` on every side (negative shrinks).
    #[inline]
    pub fn expand(&self, amount: f32) -> Rect {
        Rect {
            min: Vec2::new(self.min.x - amount, self.min.y - amount),
            max: Vec2::new(self.max.x + amount, self.max.y + amount),
        }
    }

    /// Shrinks by separate horizontal / vertical amounts.
    #[inline]
    pub fn shrink2(&self, x: f32, y: f32) -> Rect {
        Rect { min: Vec2::new(self.min.x + x, self.min.y + y), max: Vec2::new(self.max.x - x, self.max.y - y) }
    }

    /// Translates the rectangle.
    #[inline]
    pub fn translate(&self, d: Vec2) -> Rect {
        Rect { min: self.min + d, max: self.max + d }
    }

    /// Smallest rectangle containing both.
    #[inline]
    pub fn union(&self, o: &Rect) -> Rect {
        Rect { min: self.min.min(o.min), max: self.max.max(o.max) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_basics() {
        let r = Rect::from_xywh(10.0, 20.0, 100.0, 50.0);
        assert_eq!(r.width(), 100.0);
        assert_eq!(r.center(), Vec2::new(60.0, 45.0));
        assert!(r.contains(Vec2::new(10.0, 20.0)));
        assert!(!r.contains(Vec2::new(110.0, 20.0)));
        let i = r.intersect(&Rect::from_xywh(50.0, 0.0, 100.0, 30.0));
        assert_eq!(i, Rect::from_xywh(50.0, 20.0, 60.0, 10.0));
        assert!(Rect::from_xywh(0.0, 0.0, 5.0, 5.0).intersect(&Rect::from_xywh(6.0, 6.0, 1.0, 1.0)).is_empty());
    }
}
