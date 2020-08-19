use derivative::Derivative;
use derive_more::Constructor;
use num::PrimFloat;

use crate::{InfSup, InfSupResult, MathIterExt};
use crate::vector::*;

#[derive(Clone, Constructor, Copy, Debug, Derivative, Eq, PartialEq)]
#[derivative(Default(bound = "F: Copy + Default"))]
pub struct BBox<F, const N: usize> {
    pub min: Vector<F, N>,
    pub max: Vector<F, N>,
}

pub type BBox2<F> = BBox<F, 2>;
pub type BBox3<F> = BBox<F, 3>;

impl<F: PrimFloat, const N: usize> BBox<F, N> {
    pub fn from_points(iter: impl IntoIterator<Item = Vector<F, N>>) ->
        Option<Self>
    {
        match iter.into_iter().infsup() {
            InfSupResult::Empty => None,
            InfSupResult::Singleton(x) => Some(Self::new(x, x)),
            InfSupResult::InfSup(inf, sup) => Some(Self::new(inf, sup)),
        }
    }

    #[inline]
    pub fn volume(&self) -> F {
        let diam = self.max - self.min;
        diam.iter().product()
    }

    #[inline]
    pub fn contains(&self, point: &Vector<F, N>) -> bool {
        (self.min <= *point) & (*point <= self.max)
    }

    #[inline]
    pub fn intersects(&self, other: &Self) -> bool {
        let inf = self.inf(other);
        // TODO: Maybe edges shouldn't count as intersecting?
        inf.min <= inf.max
    }

    #[inline]
    pub fn inf(&self, other: &Self) -> Self {
        Self {
            min: self.min.sup(&other.min),
            max: self.max.inf(&other.max),
        }
    }

    #[inline]
    pub fn sup(&self, other: &Self) -> Self {
        Self {
            min: self.min.inf(&other.min),
            max: self.max.sup(&other.max),
        }
    }
}


impl<F: PrimFloat, const N: usize> InfSup for BBox<F, N> {
    impl_inf_sup!(Self);
}

impl<'a, F: PrimFloat + 'a, const N: usize> InfSup<&'a Self> for BBox<F, N> {
    impl_inf_sup!(&'a Self);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contains() {
        let bbox = BBox::new(vec2(0.0, 0.0), vec2(1.0, 1.0));
        assert!(bbox.contains(&vec2(0.0, 0.0)));
        assert!(bbox.contains(&vec2(1.0, 0.0)));
        assert!(!bbox.contains(&vec2(1.5, 0.0)));
        assert!(bbox.contains(&vec2(0.5, 0.5)));
        assert!(!bbox.contains(&vec2(-1.0, 0.5)));
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

        assert_eq!(bbox1.inf(&bbox2), BBox::new(p1, p2));
        assert_eq!(bbox1.sup(&bbox2), BBox::new(p0, p3));
        assert!(bbox1.intersects(&bbox2));
        assert!(bbox2.intersects(&bbox1));

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
        for point in points.iter() {
            assert!(bbox.contains(point), "{:?}", point);
        }
    }
}
