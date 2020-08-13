#![allow(clippy::needless_range_loop, clippy::too_many_arguments)]

use std::ops::*;

use base::impl_bin_ops;
use num::*;

use crate::Dot;
use crate::vector::*;

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
    pub fn iter(&self) -> impl ExactSizeIterator<Item=&Vector<F, M>> {
        self.columns.iter()
    }

    #[inline(always)]
    pub fn iter_mut(&mut self) ->
        impl ExactSizeIterator<Item = &mut Vector<F, M>>
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
    #[inline(always)]
    pub fn transpose(&self) -> Matrix<F, N, M> {
        let mut trans: Matrix<F, N, M> = Default::default();
        for i in 0..N {
            for j in 0..M {
                trans[j][i] = self[i][j];
            }
        }
        trans
    }

    /// Returns the K × L submatrix starting at a given row and column.
    // TODO: I would prefer to take row and col as consts but the
    // compiler can't support that yet (it ICEs).
    #[inline(always)]
    pub fn submatrix<const K: usize, const L: usize>(
        &self,
        row: usize,
        col: usize,
    ) -> Matrix<F, K, L> {
        let mut sub: Matrix<F, K, L> = Default::default();
        for i in 0..L {
            for j in 0..K {
                sub[i][j] = self[col + i][row + j];
            }
        }
        sub
    }
}

impl<F: Zero + Copy, const N: usize> Matrix<F, N, N> {
    #[inline(always)]
    pub fn diagonal(diag: [F; N]) -> Self {
        let mut mat: Matrix<F, N, N> = Zero::zero();
        for i in 0..N {
            mat[i][i] = diag[i];
        }
        mat
    }
}

impl<F: Zero + One + Copy, const N: usize> Matrix<F, N, N> {
    #[inline(always)]
    pub fn identity() -> Self {
        let mut ident: Matrix<F, N, N> = Zero::zero();
        for i in 0..N {
            ident[i][i] = One::one();
        }
        ident
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
    #[inline(always)]
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
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.iter().zip(other.iter()).all(|(a, b)| PartialEq::eq(a, b))
    }
}

impl<F: Zero + Copy, const M: usize, const N: usize> Zero for Matrix<F, M, N> {
    #[inline(always)]
    fn zero() -> Self {
        Self::new([Zero::zero(); N])
    }
}

impl<F: Zero + One + Copy, const N: usize> One for Matrix<F, N, N> {
    #[inline(always)]
    fn one() -> Self {
        Self::identity()
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

// Matrix--vector mul
macro_rules! impl_matvec_mul {
    ({$($lt:tt)*}, ($Lhs:ty), ($Rhs:ty)) => {
        impl<$($lt)* F: Primitive, const M: usize, const N: usize> Mul<$Rhs>
            for $Lhs
        {
            type Output = Vector<F, M>;
            #[inline(always)]
            fn mul(self, other: $Rhs) -> Self::Output {
                self.iter().zip(other.iter()).map(|(v, x)| v * x).sum()
            }
        }
    }
}

impl_matvec_mul!({}, (Matrix<F, M, N>), (Vector<F, N>));
impl_matvec_mul!({'rhs,}, (Matrix<F, M, N>), (&'rhs Vector<F, N>));
impl_matvec_mul!({'lhs,}, (&'lhs Matrix<F, M, N>), (Vector<F, N>));
impl_matvec_mul!({'lhs, 'rhs,}, (&'lhs Matrix<F, M, N>), (&'rhs Vector<F, N>));

impl<F: Primitive, const M: usize, const N: usize> Matrix<F, M, N> {
    #[inline(always)]
    pub fn transpose_mul_vec(&self, vector: &Vector<F, M>) -> Vector<F, N> {
        let mut prod: Vector<F, N> = Default::default();
        // TODO: This implementation might be slower than simply
        // constructing the tranpose and multiplying
        for i in 0..N {
            prod[i] = self[i].dot(vector);
        }
        prod
    }
}

// Matrix--matrix mul

impl<F: Primitive, const N: usize> MulAssign<Matrix<F, N, N>>
    for Matrix<F, N, N>
{
    #[inline(always)]
    fn mul_assign(&mut self, rhs: Matrix<F, N, N>) {
        let lhs = *self;
        for (i, x) in rhs.iter().enumerate() {
            self[i] = lhs * x;
        }
    }
}

impl<'rhs, F: Primitive, const N: usize> MulAssign<&'rhs Matrix<F, N, N>>
    for Matrix<F, N, N>
{
    #[inline(always)]
    fn mul_assign(&mut self, rhs: &'rhs Matrix<F, N, N>) {
        let lhs = *self;
        for (i, x) in rhs.iter().enumerate() {
            self[i] = lhs * x;
        }
    }
}

macro_rules! impl_matmat_mul {
    ({$($lt:tt)*}, ($Output:ty), ($Lhs:ty), ($Rhs:ty)) => {
        impl<
            $($lt)*
            F: Primitive,
            const M: usize,
            const N: usize,
            const K: usize,
        > Mul<$Rhs> for $Lhs {
            type Output = $Output;
            #[inline(always)]
            fn mul(self, other: $Rhs) -> Self::Output {
                let mut out = Self::Output::zero();
                for (i, x) in other.iter().enumerate() {
                    out[i] = self * x;
                }
                out
            }
        }
    }
}

impl_matmat_mul!(
    {}, (Matrix<F, M, K>),
    (Matrix<F, M, N>), (Matrix<F, N, K>)
);
impl_matmat_mul!(
    {'rhs,}, (Matrix<F, M, K>),
    (Matrix<F, M, N>), (&'rhs Matrix<F, N, K>)
);
impl_matmat_mul!(
    {'lhs,}, (Matrix<F, M, K>),
    (&'lhs Matrix<F, M, N>), (Matrix<F, N, K>)
);
impl_matmat_mul!(
    {'lhs, 'rhs,}, (Matrix<F, M, K>),
    (&'lhs Matrix<F, M, N>), (&'rhs Matrix<F, N, K>)
);

impl<F: Zero + One + Copy> Matrix3<F> {
    /// Turns a 3-dimensional matrix into an affine transformation on
    /// the homogeneous coordinate space.
    #[inline(always)]
    pub fn translate(&self, trans: Vector3<F>) -> Matrix4<F> {
        [self[0].xyz0(), self[1].xyz0(), self[2].xyz0(), trans.xyz1()].into()
    }

    #[inline(always)]
    pub fn xyz1(&self) -> Matrix4<F> {
        [
            self[0].xyz0(), self[1].xyz0(), self[2].xyz0(),
            vec4(Zero::zero(), Zero::zero(), Zero::zero(), One::one()),
        ].into()
    }
}

impl<F: Copy + Default> Matrix4<F> {
    #[inline(always)]
    pub fn xyz(&self) -> Matrix3<F> {
        self.submatrix(0, 0)
    }
}

impl<F: Copy> Matrix4<F> {
    /// The first three elements of the last column.
    #[inline(always)]
    pub fn translation(&self) -> Vector3<F> {
        self[3].xyz()
    }
}

#[cfg(test)]
mod tests {
    use crate::vector::*;
    use super::*;

    #[test]
    fn accessors() {
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
    fn matrix_ops() {
        let a: Matrix2<f32> = [
            [1.0, 0.0],
            [0.0, 1.0]].into();
        let b: Matrix2<f32> = [
            [0.0, 1.0],
            [1.0, 0.0]].into();
        assert_eq!(a, a);
        assert_ne!(a, b);
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
    fn scalar_ops() {
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

    #[test]
    fn mat_vec_mul() {
        let a: Matrix2<f32> = [
            [1.0, 0.0],
            [0.0, 1.0]].into();
        assert_eq!(a * Vector::zero(), Vector::zero());
        assert_eq!(a * &vec2(1.0, 2.0), vec2(1.0, 2.0));
        let a: Matrix2<f32> = [
            [ 0.0, 1.0],
            [-1.0, 0.0]].into();
        assert_eq!(&a * &vec2(1.0, 2.0), vec2(-2.0, 1.0));
    }

    #[test]
    fn mat_mat_mul() {
        let a: Matrix2<f32> = [
            [1.0, 0.0],
            [0.0, 1.0]].into();
        let b: Matrix2<f32> = [
            [ 0.0, 1.0],
            [-1.0, 0.0]].into();
        assert_eq!(a * b, b);
        assert_eq!(&a * b, b);
        let mut b_1 = b;
        b_1 *= a;
        assert_eq!(b_1, b);

        let c: Matrix2x3<f32> = [
            [0.0, 1.0],
            [2.0, 3.0],
            [4.0, 5.0]].into();
        assert_eq!(a * c, c);
    }

    #[test]
    fn mat0() {
        let a: Matrix<f32, 0, 0> = Default::default();
        let b: Matrix<f32, 0, 0> = Matrix::new([]);
        assert_eq!(a, b);
        assert_eq!(a + b, a - b);
    }

    #[test]
    fn mat1() {
        let a: Matrix<f32, 1, 1> = Default::default();
        let b: Matrix<f32, 1, 1> = [[1.0]].into();
        assert_eq!(a[0][0], 0.0);
        assert_eq!(b[0], vec([1.0]));
        assert_eq!(a + b, b);
        assert_eq!(a - b, -b);
        assert_eq!(a * b, a);
    }

    #[test]
    fn methods() {
        let a: Matrix2<f32> = Matrix::identity();
        assert_eq!(a, Matrix::diagonal([1.0, 1.0]));
        assert_eq!(a, a.transpose());
        let b: Matrix3x2<f32> = [
            [0.0, 1.0, 2.0],
            [3.0, 4.0, 5.0]].into();
        assert_eq!(
            b.transpose().elements(),
            [ 0.0, 3.0
            , 1.0, 4.0
            , 2.0, 5.0]);
    }

    #[test]
    fn swizzle() {
        let mut a: Matrix4<f32> = [
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 1.0]].into();
        assert_eq!(
            a.xyz().elements(),
            [ 0.0, 1.0, 0.0
            , 0.0, 0.0, 1.0
            , 1.0, 0.0, 0.0]);
        assert_eq!(a.xyz().xyz1(), a);
        assert_eq!(a.xyz().translate(Zero::zero()), a);
        assert_eq!(a.translation(), Zero::zero());

        let t = vec3(2.0, 3.0, 0.0);
        a[3] = t.xyz1();
        assert_eq!(a.translation(), t);
        assert_eq!(a.xyz().translate(t), a);

        let b: Matrix<f32, 0, 0> = a.submatrix(42, 77);
        assert_eq!(b, Matrix::new([]));
        let b: Matrix<f32, 1, 1> = a.submatrix(1, 0);
        assert_eq!(b, [[1.0]].into());
        let b: Matrix3x2<f32> = a.submatrix(1, 2);
        assert_eq!(
            b.elements(),
            [ 0.0, 0.0, 0.0
            , 3.0, 0.0, 1.0]);
    }
}
