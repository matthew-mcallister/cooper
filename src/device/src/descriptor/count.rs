use base::impl_bin_ops;
use math::Vector;
use num::Zero;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
crate struct DescriptorCounts(Vector<u32, 11>);

impl Default for DescriptorCounts {
    fn default() -> Self {
        Self(Zero::zero())
    }
}

impl From<Vector<u32, 11>> for DescriptorCounts {
    fn from(vec: Vector<u32, 11>) -> Self {
        Self(vec)
    }
}

impl From<DescriptorCounts> for Vector<u32, 11> {
    fn from(counts: DescriptorCounts) -> Self {
        counts.0
    }
}

impl DescriptorCounts {
    crate fn new() -> Self { Default::default() }

    crate fn iter(&self) ->
        impl Iterator<Item = (vk::DescriptorType, u32)> + '_
    {
        self.0.iter().enumerate()
            .map(|(i, v)| (vk::DescriptorType(i as _), *v))
    }

    crate fn iter_mut(&mut self) ->
        impl Iterator<Item = (vk::DescriptorType, &mut u32)>
    {
        self.0.iter_mut().enumerate()
            .map(|(i, v)| (vk::DescriptorType(i as _), v))
    }
}

macro_rules! impl_vec_op {
    ($Op:ident, $OpAssign:ident, $op:ident, $op_assign:ident) => {
        impl std::ops::$OpAssign for DescriptorCounts {
            fn $op_assign(&mut self, other: Self) {
                std::ops::$OpAssign::$op_assign(&mut self.0, other.0);
            }
        }

        impl<'rhs> std::ops::$OpAssign<&'rhs Self> for DescriptorCounts {
            fn $op_assign(&mut self, other: &'rhs Self) {
                std::ops::$OpAssign::$op_assign(&mut self.0, other.0);
            }
        }

        impl_bin_ops!(
            (DescriptorCounts), (DescriptorCounts), copy,
            (std::ops::$Op), (std::ops::$OpAssign), $op, $op_assign,
        );
    }
}

macro_rules! impl_scalar_op {
    ($Op:ident, $OpAssign:ident, $op:ident, $op_assign:ident) => {
        impl std::ops::$OpAssign<u32> for DescriptorCounts {
            fn $op_assign(&mut self, other: u32) {
                std::ops::$OpAssign::<u32>::$op_assign(&mut self.0, other);
            }
        }

        impl<'rhs> std::ops::$OpAssign<&'rhs u32> for DescriptorCounts {
            fn $op_assign(&mut self, other: &'rhs u32) {
                std::ops::$OpAssign::<u32>::$op_assign(&mut self.0, *other);
            }
        }

        impl_bin_ops!(
            (DescriptorCounts), (u32), copy,
            (std::ops::$Op), (std::ops::$OpAssign), $op, $op_assign,
        );
    }
}

impl_vec_op!(Add, AddAssign, add, add_assign);
impl_vec_op!(Sub, SubAssign, sub, sub_assign);
impl_scalar_op!(Mul, MulAssign, mul, mul_assign);
impl_scalar_op!(Div, DivAssign, div, div_assign);

impl std::ops::Index<vk::DescriptorType> for DescriptorCounts {
    type Output = u32;
    fn index(&self, idx: vk::DescriptorType) -> &Self::Output {
        &self.0[idx.0 as usize]
    }
}

impl std::ops::IndexMut<vk::DescriptorType> for DescriptorCounts {
    fn index_mut(&mut self, idx: vk::DescriptorType) -> &mut Self::Output {
        &mut self.0[idx.0 as usize]
    }
}

impl std::iter::Sum<(vk::DescriptorType, u32)> for DescriptorCounts {
    fn sum<I>(iter: I) -> Self
        where I: Iterator<Item = (vk::DescriptorType, u32)>
    {
        let mut counts = DescriptorCounts::default();
        for (k, v) in iter {
            counts[k] += v;
        }
        counts
    }
}

impl std::iter::FromIterator<(vk::DescriptorType, u32)> for DescriptorCounts {
    fn from_iter<I>(iter: I) -> Self
        where I: IntoIterator<Item = (vk::DescriptorType, u32)>
    {
        let mut counts = DescriptorCounts::default();
        for (k, v) in iter {
            counts[k] = v;
        }
        counts
    }
}
