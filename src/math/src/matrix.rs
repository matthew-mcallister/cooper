#![allow(clippy::needless_range_loop, clippy::too_many_arguments)]

use std::ops::*;

use derivative::Derivative;
use num::*;

use crate::vector::*;

pub trait BasicVector<F, const N: usize> = where
    F: Scalar<N>,
    Self: Sized
        + Neg<Output = Self>
        + Add<Self, Output = Self>
        + AddAssign<Self>
        + Sub<Self, Output = Self>
        + SubAssign<Self>
        + Mul<Self, Output = Self>
        + MulAssign<Self>
        + Mul<F, Output = Self>
        + MulAssign<F>
        + Div<Self, Output = Self>
        + DivAssign<Self>
        + Div<F, Output = Self>
        + DivAssign<F>
        ;

/// A SIMD-backed, column-major, dense M x N matrix meant for doing fast
/// transformations on vectors.
///
/// Indexing a matrix returns the column vector in that position, which
/// is typical in numeric code but the reverse of the mathematical
/// convention, in which the row comes first.
#[derive(Derivative)]
#[derivative(
    Clone(bound = ""),
    Copy(bound = ""),
    Debug(bound = ""),
    PartialEq(bound = "Vector<F, M>: PartialEq")
)]
#[repr(transparent)]
pub struct Matrix<F: Scalar<M>, const M: usize, const N: usize> {
    columns: [Vector<F, M>; N],
}

pub type Matrix2<F> = Matrix<F, 2, 2>;
pub type Matrix3<F> = Matrix<F, 3, 3>;
pub type Matrix4<F> = Matrix<F, 4, 4>;

pub type Matrix2x3<F> = Matrix<F, 2, 3>;
pub type Matrix2x4<F> = Matrix<F, 2, 4>;
pub type Matrix3x2<F> = Matrix<F, 3, 2>;
pub type Matrix3x4<F> = Matrix<F, 3, 4>;
pub type Matrix4x2<F> = Matrix<F, 4, 2>;
pub type Matrix4x3<F> = Matrix<F, 4, 3>;

impl<F: Scalar<M>, const M: usize, const N: usize> Matrix<F, M, N> {
    #[inline(always)]
    pub fn new(columns: [Vector<F, M>; N]) -> Self {
        Self { columns }
    }

    #[inline(always)]
    pub fn columns(&self) -> &[Vector<F, M>; N] {
        &self.columns
    }

    #[inline(always)]
    pub fn columns_mut(&mut self) -> &mut [Vector<F, M>; N] {
        &mut self.columns
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

    #[inline(always)]
    pub fn load(array: &[[F; M]; N]) -> Self {
        let mut mat = Self::default();
        for i in 0..N {
            mat[i] = Vector::load(&array[i]);
        }
        mat
    }

    #[inline(always)]
    pub fn store(self, array: &mut [[F; M]; N]) {
        for i in 0..N {
            self[i].store(&mut array[i]);
        }
    }

    #[inline(always)]
    pub fn load_rows(rows: [[F; N]; M]) -> Self {
        let mut mat = Self::default();
        for i in 0..M {
            for j in 0..N {
                mat[j][i] = rows[i][j];
            }
        }
        mat
    }

    #[inline(always)]
    pub fn to_array(self) -> [[F; M]; N] {
        self.into()
    }

    /// Returns the K × L submatrix starting at a given row and column.
    // TODO: I would prefer to take row and col as consts but the
    // compiler can't support that yet (it ICEs).
    #[inline(always)]
    pub fn submatrix<const K: usize, const L: usize>(
        self,
        row: usize,
        col: usize,
    ) -> Matrix<F, K, L>
        where F: Scalar<K>
    {
        let mut sub: Matrix<F, K, L> = Default::default();
        for i in 0..L {
            for j in 0..K {
                sub[i][j] = self[col + i][row + j];
            }
        }
        sub
    }
}

impl<F: Scalar<M> + Scalar<N>, const M: usize, const N: usize> Matrix<F, M, N>
{
    #[inline(always)]
    pub fn transpose(self) -> Matrix<F, N, M> {
        let mut trans: Matrix<F, N, M> = Default::default();
        for i in 0..N {
            for j in 0..M {
                trans[j][i] = self[i][j];
            }
        }
        trans
    }
}

impl<F: Scalar<N>, const N: usize> Matrix<F, N, N> {
    #[inline(always)]
    pub fn diagonal(diag: [F; N]) -> Self {
        let mut mat: Matrix<F, N, N> = Zero::zero();
        for i in 0..N {
            mat[i][i] = diag[i];
        }
        mat
    }

    #[inline(always)]
    pub fn identity() -> Self {
        let mut ident: Matrix<F, N, N> = Zero::zero();
        for i in 0..N {
            ident[i][i] = One::one();
        }
        ident
    }
}

impl<F: Scalar<M>, const M: usize, const N: usize> Default
    for Matrix<F, M, N>
{
    #[inline(always)]
    fn default() -> Self {
        Self::new([Vector::default(); N])
    }
}

impl<F: Scalar<M>, const M: usize, const N: usize> Zero
    for Matrix<F, M, N>
{
    #[inline(always)]
    fn zero() -> Self {
        Self::new([Vector::zero(); N])
    }
}

impl<F: Scalar<M>, const M: usize, const N: usize> AsRef<[Vector<F, M>; N]>
    for Matrix<F, M, N>
{
    #[inline(always)]
    fn as_ref(&self) -> &[Vector<F, M>; N] {
        &self.columns
    }
}

impl<F: Scalar<M>, const M: usize, const N: usize> AsMut<[Vector<F, M>; N]>
    for Matrix<F, M, N>
{
    #[inline(always)]
    fn as_mut(&mut self) -> &mut [Vector<F, M>; N] {
        &mut self.columns
    }
}

impl<I, F: Scalar<M>, const M: usize, const N: usize> Index<I>
    for Matrix<F, M, N>
    where [Vector<F, M>]: Index<I>
{
    type Output = <[Vector<F, M>] as Index<I>>::Output;
    fn index(&self, idx: I) -> &Self::Output {
        self.columns.index(idx)
    }
}

impl<I, F: Scalar<M>, const M: usize, const N: usize> IndexMut<I>
    for Matrix<F, M, N>
    where [Vector<F, M>]: IndexMut<I>
{
    fn index_mut(&mut self, idx: I) -> &mut Self::Output {
        self.columns.index_mut(idx)
    }
}

impl<F: Scalar<M>, const M: usize, const N: usize> From<[[F; M]; N]>
    for Matrix<F, M, N>
{
    fn from(array: [[F; M]; N]) -> Self {
        Self::load(&array)
    }
}

impl<F: Scalar<M>, const M: usize, const N: usize> From<Matrix<F, M, N>>
    for [[F; M]; N]
{
    fn from(mat: Matrix<F, M, N>) -> Self {
        let mut array = [[Default::default(); M]; N];
        mat.store(&mut array);
        array
    }
}

impl<F: Scalar<M>, const M: usize, const N: usize> From<[Vector<F, M>; N]>
    for Matrix<F, M, N>
{
    fn from(columns: [Vector<F, M>; N]) -> Self {
        Self { columns }
    }
}

impl<F: Scalar<M>, const M: usize, const N: usize> From<Matrix<F, M, N>>
    for [Vector<F, M>; N]
{
    fn from(mat: Matrix<F, M, N>) -> Self {
        mat.columns
    }
}

macro_rules! impl_matn {
    ($N:expr, $matn:ident, $($arg:ident),*) => {
        #[inline(always)]
        pub fn $matn<F: Scalar<$N>>($($arg: Vector<F, $N>),*) ->
            Matrix<F, $N, $N>
        {
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

macro_rules! impl_un_op {
    ($Op:ident, $op:ident) => {
        impl<F, const M: usize, const N: usize> $Op for Matrix<F, M, N>
        where
            F: Scalar<M>,
            Vector<F, M>: $Op<Output = Vector<F, M>>,
        {
            type Output = Matrix<F, M, N>;
            #[inline(always)]
            fn $op(mut self) -> Matrix<F, M, N> {
                for i in 0..N {
                    self[i] = $Op::$op(self[i]);
                }
                self
            }
        }
    }
}

impl_un_op!(Neg, neg);

macro_rules! impl_bin_op {
    ($Op:ident, $OpAssign:ident, $op:ident, $op_assign:ident) => {
        impl<F, const M: usize, const N: usize> $OpAssign for Matrix<F, M, N>
        where
            F: Scalar<M>,
            Vector<F, M>: $OpAssign,
        {
            #[inline(always)]
            fn $op_assign(&mut self, other: Matrix<F, M, N>) {
                for i in 0..N {
                    $OpAssign::$op_assign(&mut self[i], other[i]);
                }
            }
        }

        impl<F, const M: usize, const N: usize> $Op for Matrix<F, M, N>
        where
            F: Scalar<M>,
            Self: $OpAssign,
        {
            type Output = Matrix<F, M, N>;
            #[inline(always)]
            fn $op(mut self, other: Matrix<F, M, N>) -> Matrix<F, M, N> {
                $OpAssign::$op_assign(&mut self, other);
                self
            }
        }
    }
}

impl_bin_op!(Add, AddAssign, add, add_assign);
impl_bin_op!(Sub, SubAssign, sub, sub_assign);

macro_rules! impl_scalar_op {
    ($Op:ident, $OpAssign:ident, $op:ident, $op_assign:ident) => {
        impl<F, const M: usize, const N: usize> $OpAssign<F>
            for Matrix<F, M, N>
        where
            F: Scalar<M>,
            Vector<F, M>: $OpAssign<F>,
        {
            #[inline(always)]
            fn $op_assign(&mut self, scalar: F) {
                for i in 0..N {
                    $OpAssign::$op_assign(&mut self[i], scalar);
                }
            }
        }

        impl<F, const M: usize, const N: usize> $Op<F> for Matrix<F, M, N>
        where
            F: Scalar<M>,
            Self: $OpAssign<F>,
        {
            type Output = Matrix<F, M, N>;
            #[inline(always)]
            fn $op(mut self, scalar: F) -> Matrix<F, M, N> {
                $OpAssign::$op_assign(&mut self, scalar);
                self
            }
        }
    }
}

impl_scalar_op!(Mul, MulAssign, mul, mul_assign);
impl_scalar_op!(Div, DivAssign, div, div_assign);

macro_rules! impl_scalar_op_reverse {
    ($f:tt, $m:tt, $n:tt, $Op:ident, $op:ident) => {
        impl $Op<Matrix<$f, $m, $n>> for $f {
            type Output = Matrix<$f, $m, $n>;
            #[inline(always)]
            fn $op(self, mat: Matrix<$f, $m, $n>) -> Matrix<$f, $m, $n> {
                $Op::$op(mat, self)
            }
        }
    }
}

impl_scalar_op_reverse!(f32, 2, 2, Mul, mul);
impl_scalar_op_reverse!(f32, 3, 3, Mul, mul);
impl_scalar_op_reverse!(f32, 4, 4, Mul, mul);

impl<F, const M: usize, const N: usize> Mul<Vector<F, N>>
    for Matrix<F, M, N>
where
    Vector<F, M>: BasicVector<F, M>,
    Vector<F, N>: BasicVector<F, N>,
{
    type Output = Vector<F, M>;
    #[inline(always)]
    fn mul(self, vec: Vector<F, N>) -> Self::Output {
        let mut prod = Vector::zero();
        for i in 0..N {
            prod += self[i] * vec[i];
        }
        prod
    }
}

impl<F, const M: usize, const N: usize, const K: usize> Mul<Matrix<F, N, K>>
    for Matrix<F, M, N>
where
    Vector<F, N>: BasicVector<F, N>,
    Vector<F, M>: BasicVector<F, M>,
{
    type Output = Matrix<F, M, K>;
    #[inline(always)]
    fn mul(self, other: Matrix<F, N, K>) -> Self::Output {
        let mut prod = Matrix::<F, M, K>::default();
        for i in 0..K {
            prod[i] = self * other[i];
        }
        prod
    }
}

impl<F, const N: usize> MulAssign<Matrix<F, N, N>> for Matrix<F, N, N>
    where Vector<F, N>: BasicVector<F, N>
{
    #[inline(always)]
    fn mul_assign(&mut self, other: Matrix<F, N, N>) {
        *self = *self * other;
    }
}

impl Matrix3<f32> {
    /// Turns a 3-dimensional matrix into an affine transformation on
    /// the homogeneous coordinate space.
    #[inline(always)]
    pub fn translate(&self, trans: Vector3<f32>) -> Matrix4<f32> {
        [self[0].xyz0(), self[1].xyz0(), self[2].xyz0(), trans.xyz1()].into()
    }

    #[inline(always)]
    pub fn xyz1(&self) -> Matrix4<f32> {
        [
            self[0].xyz0(), self[1].xyz0(), self[2].xyz0(),
            vec4(Zero::zero(), Zero::zero(), Zero::zero(), One::one()),
        ].into()
    }
}

impl Matrix4<f32> {
    #[inline(always)]
    pub fn xyz(&self) -> Matrix3<f32> {
        self.submatrix(0, 0)
    }

    /// The first three elements of the last column.
    #[inline(always)]
    pub fn translation(&self) -> Vector3<f32> {
        self[3].xyz()
    }
}

#[cfg(test)]
mod tests {
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
        assert_eq!(m.columns(), &[vec2(0.707, 0.707), vec2(-0.707, 0.707)]);
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
            (-a).to_array(),
            [
                [-1.0,  0.0],
                [ 0.0, -1.0],
            ],
        );
        assert_eq!(
            (a + a).to_array(),
            [
                [2.0, 0.0],
                [0.0, 2.0],
            ],
        );
        assert_eq!(
            (a - b).to_array(),
            [
                [ 1.0, -1.0],
                [-1.0,  1.0],
            ],
        );
        assert_eq!(a - a, Zero::zero());
    }

    #[test]
    fn scalar_ops() {
        let a: Matrix2<f32> = [
            [1.0, 0.0],
            [0.0, 1.0]].into();
        assert_eq!(a * 1.0, a);
        assert_eq!(1.0 * a, a);
        assert_eq!(a * 0.0, Zero::zero());
        assert_eq!(0.0 * a, Zero::zero());
        assert_eq!(a * 2.0, a + a);
        assert_eq!(2.0 * a, a + a);
        assert_eq!(a * -1.0, -a);
        assert_eq!(-1.0 * a, -a);

        assert_eq!(a / 1.0, a);
        assert_eq!((a / 0.0)[0][0], f32::INFINITY);
        assert!((a / 0.0)[0][1].is_nan());
        assert_eq!(a / 2.0, a - a * 0.5);
        assert_eq!(a / -1.0, -a);
    }

    #[test]
    fn mat_vec_mul() {
        let a: Matrix2<f32> = [
            [1.0, 0.0],
            [0.0, 1.0]].into();
        assert_eq!(a * Vector::zero(), Vector::zero());
        assert_eq!(a * vec2(1.0, 2.0), vec2(1.0, 2.0));
        let a: Matrix2<f32> = [
            [ 0.0, 1.0],
            [-1.0, 0.0]].into();
        assert_eq!(a * vec2(1.0, 2.0), vec2(-2.0, 1.0));
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
    fn methods() {
        let a: Matrix2<f32> = Matrix::identity();
        assert_eq!(a, Matrix::diagonal([1.0, 1.0]));
        assert_eq!(a, a.transpose());
        let b: Matrix3x2<f32> = [
            [0.0, 1.0, 2.0],
            [3.0, 4.0, 5.0]].into();
        assert_eq!(
            b.transpose(),
            [
                [0.0, 3.0],
                [1.0, 4.0],
                [2.0, 5.0],
            ].into(),
        );
    }

    #[test]
    fn swizzle() {
        let mut a: Matrix4<f32> = [
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 1.0]].into();
        assert_eq!(
            a.xyz(),
            [
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 1.0],
                [1.0, 0.0, 0.0],
            ].into(),
        );
        assert_eq!(a.xyz().xyz1(), a);
        assert_eq!(a.xyz().translate(Zero::zero()), a);
        assert_eq!(a.translation(), Zero::zero());

        let t = vec3(2.0, 3.0, 0.0);
        a[3] = t.xyz1();
        assert_eq!(a.translation(), t);
        assert_eq!(a.xyz().translate(t), a);

        let b: Matrix3x2<f32> = a.submatrix(1, 2);
        assert_eq!(
            b,
            [
                [0.0, 0.0, 0.0],
                [3.0, 0.0, 1.0],
            ].into(),
        );
    }
}
