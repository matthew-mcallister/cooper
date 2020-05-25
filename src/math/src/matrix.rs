use std::ops::*;

use base::impl_bin_ops;
use prelude::num::*;

use crate::vector::Vector;

/// A column-major, dense, M x N matrix meant for doing fast
/// transformations and solving small systems of equations.
///
/// Indexing a matrix returns the column vector in that position. As a
/// consequence, indexing a particular element requires the reverse of
/// the usual notation. For example, `m[2][1]` is the element at column
/// 2, row 1 (numbered from zero).
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct Matrix<F, const M: usize, const N: usize> {
    columns: [Vector<F, M>; N],
}

pub type Matrix2<F> = Matrix<F, 2, 2>;
pub type Matrix3<F> = Matrix<F, 3, 3>;
pub type Matrix4<F> = Matrix<F, 4, 4>;
pub type Matrix5<F> = Matrix<F, 5, 5>;
pub type Matrix6<F> = Matrix<F, 6, 6>;
pub type Matrix7<F> = Matrix<F, 7, 7>;
pub type Matrix8<F> = Matrix<F, 8, 8>;
pub type Matrix9<F> = Matrix<F, 9, 9>;

pub type Matrix2x3<F> = Matrix<F, 2, 3>;
pub type Matrix2x4<F> = Matrix<F, 2, 4>;
pub type Matrix3x2<F> = Matrix<F, 3, 2>;
pub type Matrix3x4<F> = Matrix<F, 3, 4>;
pub type Matrix4x2<F> = Matrix<F, 4, 2>;
pub type Matrix4x3<F> = Matrix<F, 4, 3>;

impl<F, const M: usize, const N: usize> Matrix<F, M, N> {
    #[inline(always)]
    pub fn new(columns: [Vector<F, M>; N]) -> Self {
        columns.into()
    }

    #[inline(always)]
    pub fn columns(&self) -> &[Vector<F, M>; N] {
        &self.columns
    }

    #[inline(always)]
    pub fn columns_mut(&mut self) -> &mut [Vector<F, M>; N] {
        &mut self.columns
    }

    // TODO: Ought to be [F; M * N]
    #[inline(always)]
    pub fn elements(&self) -> &[F] {
        unsafe { std::slice::from_raw_parts(self as *const _ as _, M * N) }
    }

    #[inline(always)]
    pub fn elements_mut(&mut self) -> &mut [F] {
        unsafe { std::slice::from_raw_parts_mut(self as *mut _ as _, M * N) }
    }

    #[inline(always)]
    pub fn iter(&self) ->
        impl Iterator<Item=&Vector<F, M>> + ExactSizeIterator
    {
        self.columns.iter()
    }

    #[inline(always)]
    pub fn iter_mut(&mut self) ->
        impl Iterator<Item = &mut Vector<F, M>> + ExactSizeIterator
    {
        self.columns.iter_mut()
    }
}

impl<F: Copy + Default, const M: usize, const N: usize> Matrix<F, M, N> {
    #[inline(always)]
    pub fn from_rows(rows: [Vector<F, N>; M]) -> Self {
        let mut mat = Matrix::default();
        for i in 0..M {
            for j in 0..N {
                mat[j][i] = rows[i][j];
            }
        }
        mat
    }
}

impl<F: std::fmt::Debug, const M: usize, const N: usize> std::fmt::Debug
    for Matrix<F, M, N>
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.columns[..].fmt(f)
    }
}

macro_rules! impl_matn {
    ($N:expr, $matn:ident, $($arg:ident),*) => {
        #[inline(always)]
        pub fn $matn<F>($($arg: Vector<F, $N>,)*) -> Matrix<F, $N, $N> {
            [$($arg,)*].into()
        }
    }
}

impl_matn!(2, mat2, a, b);
impl_matn!(3, mat3, a, b, c);
impl_matn!(4, mat4, a, b, c, d);
impl_matn!(5, mat5, a, b, c, d, e);
impl_matn!(6, mat6, a, b, c, d, e, f);
impl_matn!(7, mat7, a, b, c, d, e, f, g);
impl_matn!(8, mat8, a, b, c, d, e, f, g, h);
impl_matn!(9, mat9, a, b, c, d, e, f, g, h, i);

impl<F: Copy + Default, const M: usize, const N: usize> Default
    for Matrix<F, M, N>
{
    fn default() -> Self {
        Self::new([Default::default(); N])
    }
}

impl<F, const M: usize, const N: usize> AsRef<[Vector<F, M>; N]>
    for Matrix<F, M, N>
{
    #[inline(always)]
    fn as_ref(&self) -> &[Vector<F, M>; N] {
        &self.columns
    }
}

impl<F, const M: usize, const N: usize> AsMut<[Vector<F, M>; N]>
    for Matrix<F, M, N>
{
    #[inline(always)]
    fn as_mut(&mut self) -> &mut [Vector<F, M>; N] {
        &mut self.columns
    }
}

impl<F, const M: usize, const N: usize> From<[[F; M]; N]> for Matrix<F, M, N> {
    #[inline(always)]
    fn from(cols: [[F; M]; N]) -> Self {
        // Yes, this sucks.
        let columns = unsafe { std::ptr::read(&cols as *const _ as _) };
        std::mem::forget(cols);
        Self { columns }
    }
}

impl<F, const M: usize, const N: usize> From<[Vector<F, M>; N]>
    for Matrix<F, M, N>
{
    #[inline(always)]
    fn from(columns: [Vector<F, M>; N]) -> Self {
        Self { columns }
    }
}

impl<F: PartialEq, const M: usize, const N: usize> PartialEq
    for Matrix<F, M, N>
{
    fn ne(&self, other: &Self) -> bool {
        self.iter().zip(other.iter()).any(|(a, b)| PartialEq::ne(a, b))
    }

    fn eq(&self, other: &Self) -> bool {
        !PartialEq::ne(self, other)
    }
}

impl<F: Zero + Copy, const M: usize, const N: usize> Zero for Matrix<F, M, N> {
    #[inline(always)]
    fn zero() -> Self {
        Self::new([Zero::zero(); N])
    }
}

// TODO: impl From<[F; M * N]> and FromIterator

derive_index!(
    (F, const M: usize, const N: usize),
    Matrix<F, M, N>, columns, [Vector<F, M>],
);

impl_un_op!(
    {F: PrimSigned, const M: usize, const N: usize},
    (Matrix<F, M, N>), Neg, neg
);
impl_un_op!(
    {F: PrimInt, const M: usize, const N: usize},
    (Matrix<F, M, N>), Not, not
);

impl_bin_op!(
    {F: Primitive, const M: usize, const N: usize}, (Matrix<F, M, N>),
    Add, AddAssign, add, add_assign
);
impl_bin_op!(
    {F: Primitive, const M: usize, const N: usize}, (Matrix<F, M, N>),
    Sub, SubAssign, sub, sub_assign
);
// Mul is not implemented because it might be confused with matrix
// multiplication. Div and Rem are not implemented for consistency.

impl_scalar_op!(
    {F: Primitive, const M: usize, const N: usize}, (Matrix<F, M, N>), (F),
    Mul, MulAssign, mul, mul_assign
);
impl_scalar_op!(
    {F: Primitive, const M: usize, const N: usize}, (Matrix<F, M, N>), (F),
    Div, DivAssign, div, div_assign
);
impl_scalar_op!(
    {F: Primitive, const M: usize, const N: usize}, (Matrix<F, M, N>), (F),
    Rem, RemAssign, rem, rem_assign
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accessor_test() {
        let c = [[0.707, 0.707], [-0.707, 0.707]];
        let (u, v) = (c[0].into(), c[1].into());
        let m: Matrix2<f32> = mat2(u, v);
        assert_eq!(m[0], u);
        assert_eq!(m[1], v);
        assert_eq!(m, c.into());
        assert_eq!(m[0][1], c[0][1]);
        assert_eq!(m[1][1], c[1][1]);
        assert_eq!(&m[..], [u, v]);
        assert_eq!(m.elements(), [0.707, 0.707, -0.707, 0.707]);
    }

    #[test]
    #[cfg(test)]
    fn matrix_ops_test() {
        let a: Matrix2<f32> = [
            [1.0, 0.0],
            [0.0, 1.0]].into();
        let b: Matrix2<f32> = [
            [0.0, 1.0],
            [1.0, 0.0]].into();
        assert_eq!(
            (-a).elements(),
            &[-1.0,  0.0
            ,  0.0, -1.0]);
        assert_eq!(
            (a + a).elements(),
            &[2.0, 0.0
            , 0.0, 2.0]);
        assert_eq!(
            (a - b).elements(),
            &[ 1.0, -1.0
            , -1.0,  1.0]);
        assert_eq!(a - a, Zero::zero());
    }

    #[test]
    fn scalar_ops_test() {
        let a: Matrix2<f32> = [
            [1.0, 0.0],
            [0.0, 1.0]].into();
        assert_eq!(a * 1.0, a);
        assert_eq!(a * 0.0, Zero::zero());
        assert_eq!(a * 2.0, a + a);
        assert_eq!(a * -1.0, -a);

        assert_eq!(a / 1.0, a);
        assert_eq!((a / 0.0)[0][0], f32::INFINITY);
        assert!((a / 0.0)[0][1].is_nan());
        assert_eq!(a / 2.0, a - a * 0.5);
        assert_eq!(a / -1.0, -a);

        assert_eq!(a % 1.0, Zero::zero());
        assert!((a % 0.0).elements().iter().all(|x| x.is_nan()));
        assert_eq!(a % 2.0, a);
        assert_eq!(a % -1.0, Zero::zero());
    }
}
