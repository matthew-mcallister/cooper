#![allow(clippy::needless_range_loop, clippy::too_many_arguments)]

use std::fmt;
use std::ops::*;

use base::num::*;
use derivative::Derivative;
use derive_more::{AsMut, AsRef, From, Into};

use crate::vector::*;

/// A SIMD-backed, column-major, dense M x N matrix meant for doing fast
/// transformations on vectors.
///
/// Indexing a matrix returns the column vector in that position, which
/// is typical in numeric code but the reverse of the mathematical
/// convention, in which the row comes first.
// TODO: Override alignment for 2x2 and 4x4?
#[derive(AsRef, AsMut, Derivative, From, Into)]
#[derivative(
    Clone(bound = ""),
    Copy(bound = ""),
    Debug(bound = ""),
    PartialEq(bound = "f32: SimdArray<M>")
)]
#[repr(transparent)]
pub struct Matrix<const M: usize, const N: usize>
where
    f32: SimdArray<M>,
{
    columns: [Vector<M>; N],
}

pub type Matrix2 = Matrix<2, 2>;
pub type Matrix3 = Matrix<3, 3>;
pub type Matrix4 = Matrix<4, 4>;

pub type Matrix2x3 = Matrix<2, 3>;
pub type Matrix2x4 = Matrix<2, 4>;
pub type Matrix3x2 = Matrix<3, 2>;
pub type Matrix3x4 = Matrix<3, 4>;
pub type Matrix4x2 = Matrix<4, 2>;
pub type Matrix4x3 = Matrix<4, 3>;

// TODO: This is adequate for debugging but doesn't obey proper
// indentation...
impl<const M: usize, const N: usize> fmt::Display for Matrix<M, N>
where
    f32: SimdArray<M>,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "[")?;
        for i in 0..N {
            writeln!(f, "    {},", self[i])?;
        }
        write!(f, "]")
    }
}

impl<const M: usize, const N: usize> Matrix<M, N>
where
    f32: SimdArray<M>,
{
    #[inline(always)]
    pub fn new(columns: [Vector<M>; N]) -> Self {
        Self { columns }
    }

    #[inline(always)]
    pub fn columns(&self) -> &[Vector<M>; N] {
        &self.columns
    }

    #[inline(always)]
    pub fn columns_mut(&mut self) -> &mut [Vector<M>; N] {
        &mut self.columns
    }

    #[inline(always)]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &Vector<M>> {
        self.columns.iter()
    }

    #[inline(always)]
    pub fn iter_mut(&mut self) -> impl ExactSizeIterator<Item = &mut Vector<M>> {
        self.columns.iter_mut()
    }

    #[inline(always)]
    pub fn load(array: &[[f32; M]; N]) -> Self {
        let mut mat = Self::default();
        for i in 0..N {
            mat[i] = Vector::load(&array[i]);
        }
        mat
    }

    #[inline(always)]
    pub fn store(self, array: &mut [[f32; M]; N]) {
        for i in 0..N {
            self[i].store(&mut array[i]);
        }
    }

    #[inline(always)]
    pub fn load_rows(rows: [[f32; N]; M]) -> Self {
        let mut mat = Self::default();
        for i in 0..M {
            for j in 0..N {
                mat[j][i] = rows[i][j];
            }
        }
        mat
    }

    #[inline(always)]
    pub fn to_array(self) -> [[f32; M]; N] {
        self.into()
    }

    /// Returns the K × L submatrix starting at a given row and column.
    // TODO: I would prefer to take row and col as consts but the
    // compiler can't support that yet (it ICEs).
    #[inline(always)]
    pub fn submatrix<const K: usize, const L: usize>(self, row: usize, col: usize) -> Matrix<K, L>
    where
        f32: SimdArray<K>,
    {
        let mut sub: Matrix<K, L> = Default::default();
        for i in 0..L {
            for j in 0..K {
                sub[i][j] = self[col + i][row + j];
            }
        }
        sub
    }
}

impl<const M: usize, const N: usize> Matrix<M, N>
where
    f32: SimdArray<M> + SimdArray<N>,
{
    #[inline(always)]
    pub fn transpose(self) -> Matrix<N, M> {
        let mut trans: Matrix<N, M> = Default::default();
        for i in 0..N {
            for j in 0..M {
                trans[j][i] = self[i][j];
            }
        }
        trans
    }
}

impl<const N: usize> Matrix<N, N>
where
    f32: SimdArray<N>,
{
    #[inline(always)]
    pub fn diagonal(diag: [f32; N]) -> Self {
        let mut mat: Matrix<N, N> = Zero::zero();
        for i in 0..N {
            mat[i][i] = diag[i];
        }
        mat
    }

    #[inline(always)]
    pub fn identity() -> Self {
        let mut ident: Matrix<N, N> = Zero::zero();
        for i in 0..N {
            ident[i][i] = One::one();
        }
        ident
    }
}

impl<const M: usize, const N: usize> Default for Matrix<M, N>
where
    f32: SimdArray<M>,
{
    #[inline(always)]
    fn default() -> Self {
        Self::new([Vector::default(); N])
    }
}

impl<const M: usize, const N: usize> Zero for Matrix<M, N>
where
    f32: SimdArray<M>,
{
    #[inline(always)]
    fn zero() -> Self {
        Self::new([Vector::zero(); N])
    }
}

impl<const M: usize> One for Matrix<M, M>
where
    f32: SimdArray<M>,
{
    #[inline(always)]
    fn one() -> Self {
        let mut m = Self::zero();
        for i in 0..M {
            m[i][i] = 1.0;
        }
        m
    }
}

impl<I, const M: usize, const N: usize> Index<I> for Matrix<M, N>
where
    f32: SimdArray<M>,
    [Vector<M>]: Index<I>,
{
    type Output = <[Vector<M>] as Index<I>>::Output;
    fn index(&self, idx: I) -> &Self::Output {
        self.columns.index(idx)
    }
}

impl<I, const M: usize, const N: usize> IndexMut<I> for Matrix<M, N>
where
    f32: SimdArray<M>,
    [Vector<M>]: IndexMut<I>,
{
    fn index_mut(&mut self, idx: I) -> &mut Self::Output {
        self.columns.index_mut(idx)
    }
}

impl<const M: usize, const N: usize> From<[[f32; M]; N]> for Matrix<M, N>
where
    f32: SimdArray<M>,
{
    fn from(array: [[f32; M]; N]) -> Self {
        Self::load(&array)
    }
}

impl<const M: usize, const N: usize> From<Matrix<M, N>> for [[f32; M]; N]
where
    f32: SimdArray<M>,
{
    fn from(mat: Matrix<M, N>) -> Self {
        let mut array = [[Default::default(); M]; N];
        mat.store(&mut array);
        array
    }
}

macro_rules! impl_matn {
    ($N:expr, $matn:ident, $($arg:ident),*) => {
        #[inline(always)]
        pub fn $matn($($arg: Vector<$N>),*) -> Matrix<$N, $N> {
            [$($arg,)*].into()
        }
    }
}

impl_matn!(2, mat2, a, b);
impl_matn!(3, mat3, a, b, c);
impl_matn!(4, mat4, a, b, c, d);

macro_rules! impl_un_op {
    ($Op:ident, $op:ident) => {
        impl<const M: usize, const N: usize> $Op for Matrix<M, N>
        where
            f32: SimdArray<M>,
        {
            type Output = Matrix<M, N>;
            #[inline(always)]
            fn $op(mut self) -> Matrix<M, N> {
                for i in 0..N {
                    self[i] = $Op::$op(self[i]);
                }
                self
            }
        }
    };
}

impl_un_op!(Neg, neg);

macro_rules! impl_bin_op {
    ($Op:ident, $OpAssign:ident, $op:ident, $op_assign:ident) => {
        impl<const M: usize, const N: usize> $OpAssign for Matrix<M, N>
        where
            f32: SimdArray<M>,
        {
            #[inline(always)]
            fn $op_assign(&mut self, other: Matrix<M, N>) {
                for i in 0..N {
                    $OpAssign::$op_assign(&mut self[i], other[i]);
                }
            }
        }

        impl<const M: usize, const N: usize> $Op for Matrix<M, N>
        where
            f32: SimdArray<M>,
        {
            type Output = Matrix<M, N>;
            #[inline(always)]
            fn $op(mut self, other: Matrix<M, N>) -> Matrix<M, N> {
                $OpAssign::$op_assign(&mut self, other);
                self
            }
        }
    };
}

impl_bin_op!(Add, AddAssign, add, add_assign);
impl_bin_op!(Sub, SubAssign, sub, sub_assign);

macro_rules! impl_scalar_op {
    ($Op:ident, $OpAssign:ident, $op:ident, $op_assign:ident) => {
        impl<const M: usize, const N: usize> $OpAssign<f32> for Matrix<M, N>
        where
            f32: SimdArray<M>,
        {
            #[inline(always)]
            fn $op_assign(&mut self, scalar: f32) {
                for i in 0..N {
                    $OpAssign::$op_assign(&mut self[i], scalar);
                }
            }
        }

        impl<const M: usize, const N: usize> $Op<f32> for Matrix<M, N>
        where
            f32: SimdArray<M>,
        {
            type Output = Matrix<M, N>;
            #[inline(always)]
            fn $op(mut self, scalar: f32) -> Matrix<M, N> {
                $OpAssign::$op_assign(&mut self, scalar);
                self
            }
        }
    };
}

impl_scalar_op!(Mul, MulAssign, mul, mul_assign);
impl_scalar_op!(Div, DivAssign, div, div_assign);

impl<const M: usize, const N: usize> Mul<Matrix<M, N>> for f32
where
    f32: SimdArray<M> + SimdArray<N>,
{
    type Output = Matrix<M, N>;
    #[inline(always)]
    fn mul(self, mat: Matrix<M, N>) -> Matrix<M, N> {
        Mul::mul(mat, self)
    }
}

impl<const M: usize, const N: usize> Mul<Vector<N>> for Matrix<M, N>
where
    f32: SimdArray<M> + SimdArray<N>,
{
    type Output = Vector<M>;
    #[inline(always)]
    fn mul(self, vec: Vector<N>) -> Self::Output {
        let mut prod = Vector::zero();
        for i in 0..N {
            prod += self[i] * vec[i];
        }
        prod
    }
}

impl<const M: usize, const N: usize, const K: usize> Mul<Matrix<N, K>> for Matrix<M, N>
where
    f32: SimdArray<M> + SimdArray<N>,
{
    type Output = Matrix<M, K>;
    #[inline(always)]
    fn mul(self, other: Matrix<N, K>) -> Self::Output {
        let mut prod = Matrix::default();
        for i in 0..K {
            prod[i] = self * other[i];
        }
        prod
    }
}

impl<const N: usize> MulAssign<Matrix<N, N>> for Matrix<N, N>
where
    f32: SimdArray<N>,
{
    #[inline(always)]
    fn mul_assign(&mut self, other: Matrix<N, N>) {
        *self = *self * other;
    }
}

impl<const M: usize, const N: usize> Matrix<M, N>
where
    f32: SimdArray<M> + SimdArray<N>,
{
    /// Multiplies with a diagonal matrix on the right.
    #[inline(always)]
    pub fn scale(mut self, diag: Vector<N>) -> Self {
        for i in 0..N {
            self[i] *= diag[i];
        }
        self
    }
}

impl Matrix3 {
    /// Turns a 3-dimensional matrix into an affine transformation on
    /// the homogeneous coordinate space.
    #[inline(always)]
    pub fn translate(self, trans: Vector3) -> Matrix4 {
        [self[0].xyz0(), self[1].xyz0(), self[2].xyz0(), trans.xyz1()].into()
    }

    #[inline(always)]
    pub fn xyz1(self) -> Matrix4 {
        [
            self[0].xyz0(),
            self[1].xyz0(),
            self[2].xyz0(),
            vec4(Zero::zero(), Zero::zero(), Zero::zero(), One::one()),
        ]
        .into()
    }

    /// Constructs an orientation matrix. The last column will point in
    /// the given direction. The "down" vector is used to construct the
    /// other two columns; imagining it as the direction of gravity, the
    /// first column will point to the right and the second column will
    /// point towards the down vector.
    ///
    /// Both `dir` and `down` must be unit vectors and cannot be
    /// parallel.
    #[inline(always)]
    pub fn orientation(dir: Vector3, down: Vector3) -> Self {
        let below = (down - dir * dir.dot(down)).normalized();
        let right = below.cross(dir);
        Self::new([right, below, dir])
    }
}

impl Matrix4 {
    #[inline(always)]
    pub fn xyz(&self) -> Matrix3 {
        self.submatrix(0, 0)
    }

    /// The first three elements of the last column.
    #[inline(always)]
    pub fn translation(&self) -> Vector3 {
        self[3].xyz()
    }

    /// Returns a perspective (frustum) projection matrix with z ranging
    /// from 0 (near) to +1 (far).
    #[inline(always)]
    pub fn perspective(z_near: f32, z_far: f32, tan_x: f32, tan_y: f32) -> Self {
        let z_r = 1.0 / (z_far - z_near);
        let z_0 = z_near * z_r;
        let (t_x, t_y) = (1.0 / tan_x, 1.0 / tan_y);
        Self::from([
            [t_x, 0.0, 0.0, 0.0],
            [0.0, t_y, 0.0, 0.0],
            [0.0, 0.0, z_r, 1.0],
            [0.0, 0.0, z_0, 0.0],
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accessors() {
        let c = [[0.707, 0.707], [-0.707, 0.707]];
        let (u, v) = (c[0].into(), c[1].into());
        let m: Matrix2 = mat2(u, v);
        assert_eq!(m[0], u);
        assert_eq!(m[1], v);
        assert_eq!(m, c.into());
        assert_eq!(m[0][1], c[0][1]);
        assert_eq!(m[1][1], c[1][1]);
        assert_eq!(&m[..], [u, v]);
        assert_eq!(m.columns(), &[vec2(0.707, 0.707), vec2(-0.707, 0.707)]);
        assert_eq!(m.as_ref(), &[vec2(0.707, 0.707), vec2(-0.707, 0.707)]);
    }

    #[test]
    fn matrix_ops() {
        let a: Matrix2 = [[1.0, 0.0], [0.0, 1.0]].into();
        let b: Matrix2 = [[0.0, 1.0], [1.0, 0.0]].into();
        assert_eq!(a, a);
        assert_ne!(a, b);
        assert_eq!((-a).to_array(), [[-1.0, 0.0], [0.0, -1.0],],);
        assert_eq!((a + a).to_array(), [[2.0, 0.0], [0.0, 2.0],],);
        assert_eq!((a - b).to_array(), [[1.0, -1.0], [-1.0, 1.0],],);
        assert_eq!(a - a, Zero::zero());
    }

    #[test]
    fn scalar_ops() {
        let a: Matrix2 = [[1.0, 0.0], [0.0, 1.0]].into();
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
        let a: Matrix2 = [[1.0, 0.0], [0.0, 1.0]].into();
        assert_eq!(a * Vector::zero(), Vector::zero());
        assert_eq!(a * vec2(1.0, 2.0), vec2(1.0, 2.0));
        let a: Matrix2 = [[0.0, 1.0], [-1.0, 0.0]].into();
        assert_eq!(a * vec2(1.0, 2.0), vec2(-2.0, 1.0));
    }

    #[test]
    fn mat_mat_mul() {
        let a: Matrix2 = [[1.0, 0.0], [0.0, 1.0]].into();
        let b: Matrix2 = [[0.0, 1.0], [-1.0, 0.0]].into();
        assert_eq!(a * b, b);
        let mut b_1 = b;
        b_1 *= a;
        assert_eq!(b_1, b);

        let c: Matrix2x3 = [[0.0, 1.0], [2.0, 3.0], [4.0, 5.0]].into();
        assert_eq!(a * c, c);
    }

    #[test]
    fn methods() {
        let a: Matrix2 = Matrix::identity();
        assert_eq!(a, Matrix::diagonal([1.0, 1.0]));
        assert_eq!(a, a.transpose());
        let b: Matrix3x2 = [[0.0, 1.0, 2.0], [3.0, 4.0, 5.0]].into();
        assert_eq!(b.transpose(), [[0.0, 3.0], [1.0, 4.0], [2.0, 5.0],].into(),);

        assert_eq!(
            b.scale(vec2(-1.0, 1.0)),
            [[0.0, -1.0, -2.0], [3.0, 4.0, 5.0]].into()
        );
    }

    #[test]
    fn swizzle() {
        let mut a: Matrix4 = [
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]
        .into();
        assert_eq!(
            a.xyz(),
            [[0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0],].into(),
        );
        assert_eq!(a.xyz().xyz1(), a);
        assert_eq!(a.xyz().translate(Zero::zero()), a);
        assert_eq!(a.translation(), Zero::zero());

        let t = vec3(2.0, 3.0, 0.0);
        a[3] = t.xyz1();
        assert_eq!(a.translation(), t);
        assert_eq!(a.xyz().translate(t), a);

        let b: Matrix3x2 = a.submatrix(1, 2);
        assert_eq!(b, [[0.0, 0.0, 0.0], [3.0, 0.0, 1.0],].into(),);
    }
}
