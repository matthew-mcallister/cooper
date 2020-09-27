use derive_more::Constructor;

use crate::{InfSup, InfSupResult, MathItertools};
use crate::vector::*;

#[derive(Clone, Constructor, Copy, Debug, Default, PartialEq)]
pub struct BBox<const N: usize>
    where f32: SimdArray<N>
{
    pub min: Vector<N>,
    pub max: Vector<N>,
}

pub type BBox2 = BBox<2>;
pub type BBox3 = BBox<3>;

impl<const N: usize> BBox<N>
    where f32: SimdArray<N>
{
    pub fn from_points(iter: impl IntoIterator<Item = Vector<N>>) ->
        Option<Self>
    {
        match iter.into_iter().inf_sup() {
            InfSupResult::Empty => None,
            InfSupResult::Singleton(x) => Some(Self::new(x, x)),
            InfSupResult::InfSup(inf, sup) => Some(Self::new(inf, sup)),
        }
    }

    #[inline]
    pub fn volume(self) -> f32 {
        (self.max - self.min).product()
    }

    #[inline]
    pub fn contains(self, point: Vector<N>) -> bool {
        self.min.le(point) & point.le(self.max)
    }

    #[inline]
    pub fn intersects(self, other: Self) -> bool {
        let inf = self.inf(other);
        // TODO: Maybe edges shouldn't count as intersecting?
        inf.min.le(inf.max)
    }

    #[inline]
    pub fn inf(self, other: Self) -> Self {
        Self {
            min: self.min.sup(other.min),
            max: self.max.inf(other.max),
        }
    }

    #[inline]
    pub fn sup(self, other: Self) -> Self {
        Self {
            min: self.min.inf(other.min),
            max: self.max.sup(other.max),
        }
    }
}


impl<const N: usize> InfSup for BBox<N>
    where f32: SimdArray<N>
{
    impl_inf_sup!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contains() {
        let bbox = BBox::new(vec2(0.0, 0.0), vec2(1.0, 1.0));
        assert!(bbox.contains(vec2(0.0, 0.0)));
        assert!(bbox.contains(vec2(1.0, 0.0)));
        assert!(!bbox.contains(vec2(1.5, 0.0)));
        assert!(bbox.contains(vec2(0.5, 0.5)));
        assert!(!bbox.contains(vec2(-1.0, 0.5)));
    }

    #[test]
    fn volume() {
        assert_eq!(BBox::new(vec2(1.0, 1.0), vec2(2.0, 3.0)).volume(), 2.0);
        assert_eq!(
            BBox::new(vec3(1.0, 1.0, 1.0), vec3(2.0, 3.0, 4.0)).volume(),
            6.0,
        );
    }

    #[test]
    fn intersects() {
        let bbox = BBox::new(vec2(0.0, 0.0), vec2(1.0, 1.0));
        assert!(bbox.intersects(bbox));
        assert!(!bbox.intersects(BBox::new(vec2(-2.0, 2.0), vec2(-1.0, 3.0))));
        assert!(bbox.intersects(BBox::new(vec2(1.0, 1.0), vec2(2.0, 2.0))));
    }

    #[test]
    fn inf_sup() {
        let (p0, p1, p2, p3) = (
            vec2(0.0, 0.0),
            vec2(1.0, 1.0),
            vec2(2.0, 2.0),
            vec2(3.0, 3.0),
        );
        let bbox1 = BBox::new(p0, p2);
        let bbox2 = BBox::new(p1, p3);

        assert_eq!(bbox1.inf(bbox2), BBox::new(p1, p2));
        assert_eq!(bbox1.sup(bbox2), BBox::new(p0, p3));
        assert!(bbox1.intersects(bbox2));
        assert!(bbox2.intersects(bbox1));

        let boxes = [
            BBox::new(vec2(0.0, 0.0), vec2(2.0, 1.0)),
            BBox::new(vec2(-1.0, -1.0), vec2(0.0, 1.2)),
            BBox::new(vec2(-0.5, 0.5), vec2(1.5, 1.0)),
            BBox::new(vec2(1.0, -0.2), vec2(1.2, 0.2)),
        ];
        assert_eq!(boxes.iter().copied().sup(),
            Some(BBox::new(vec2(-1.0, -1.0), vec2(2.0, 1.2))));
    }

    #[test]
    fn construct() {
        let points = &[vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(0.5, 0.5)];
        let bbox = BBox::from_points(points.iter().copied()).unwrap();
        assert_eq!(bbox, BBox::new(vec2(0.0, 0.0), vec2(1.0, 0.5)));
        for &point in points.iter() {
            assert!(bbox.contains(point), "{:?}", point);
        }
    }
}
