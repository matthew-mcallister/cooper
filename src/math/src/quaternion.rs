use std::ops::*;

use derivative::Derivative;
use derive_more::{
    Add, AddAssign, From, Index, IndexMut, Into, Neg, Sub, SubAssign,
};
use num::{One, Zero};

use crate::matrix::*;
use crate::vector::*;

#[derive(
    Add, AddAssign, Derivative, From, Index, IndexMut, Into, Neg, Sub,
    SubAssign,
)]
#[derivative(
    Clone(bound = ""),
    Copy(bound = ""),
    Debug(bound = ""),
    Default(bound = ""),
    PartialEq(bound = ""),
)]
pub struct Quaternion(Vector4);

impl Quaternion {
    #[inline(always)]
    pub fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self([x, y, z, w].into())
    }

    #[inline(always)]
    pub fn splat(scalar: f32) -> Self {
        Self(Vector::splat(scalar))
    }

    #[inline(always)]
    pub fn load(src: &[f32; 4]) -> Self {
        Self(Vector::load(src))
    }

    #[inline(always)]
    pub fn store(self, dst: &mut [f32; 4]) {
        self.0.store(dst)
    }

    #[inline(always)]
    pub fn i() -> Self {
        Self::new(1.0, 0.0, 0.0, 0.0)
    }

    #[inline(always)]
    pub fn j() -> Self {
        Self::new(0.0, 1.0, 0.0, 0.0)
    }

    #[inline(always)]
    pub fn k() -> Self {
        Self::new(0.0, 0.0, 1.0, 0.0)
    }
}

impl From<Vector3> for Quaternion {
    fn from(v: Vector3) -> Self {
        Self(v.xyz_())
    }
}

impl From<Quaternion> for Vector3 {
    fn from(q: Quaternion) -> Self {
        q.0.xyz()
    }
}

impl Quaternion {
    #[inline(always)]
    pub fn x(&self) -> f32 {
        self.0.x()
    }

    #[inline(always)]
    pub fn y(&self) -> f32 {
        self.0.y()
    }

    #[inline(always)]
    pub fn z(&self) -> f32 {
        self.0.z()
    }

    #[inline(always)]
    pub fn w(&self) -> f32 {
        self.0.w()
    }
}

impl Zero for Quaternion {
    #[inline(always)]
    fn zero() -> Self {
        Self::splat(0.0)
    }
}

impl One for Quaternion {
    #[inline(always)]
    fn one() -> Self {
        Self::new(0.0, 0.0, 0.0, 1.0)
    }
}

macro_rules! impl_scalar_op {
    ($Op:ident, $OpAssign:ident, $op:ident, $op_assign:ident) => {
        impl $Op<f32> for Quaternion {
            type Output = Quaternion;
            #[inline(always)]
            fn $op(self, scalar: f32) -> Quaternion {
                Quaternion($Op::$op(self.0, scalar))
            }
        }

        impl $OpAssign<f32> for Quaternion {
            #[inline(always)]
            fn $op_assign(&mut self, scalar: f32) {
                $OpAssign::$op_assign(&mut self.0, scalar);
            }
        }
    }
}

impl_scalar_op!(Mul, MulAssign, mul, mul_assign);
impl_scalar_op!(Div, DivAssign, div, div_assign);

impl Mul<Quaternion> for f32 {
    type Output = Quaternion;
    #[inline(always)]
    fn mul(self, quat: Quaternion) -> Quaternion {
        Mul::mul(quat, self)
    }
}

impl Quaternion {
    #[inline(always)]
    pub fn conjugate(mut self) -> Self {
        self = -self;
        self[3] = -self[3];
        self
    }

    #[inline(always)]
    pub fn norm_sq(self) -> f32 {
        self.0.length_sq()
    }

    #[inline(always)]
    pub fn norm(self) -> f32 {
        self.norm_sq().sqrt()
    }

    #[inline(always)]
    pub fn normalize(&mut self) -> f32 {
        let norm = self.norm();
        *self /= norm;
        norm
    }

    #[inline(always)]
    pub fn normalized(self) -> Self {
        self / self.norm()
    }

    #[inline(always)]
    pub fn inverse(self) -> Self {
        self.conjugate() / self.norm_sq()
    }

    /// Constructs a rotation matrix from a *unit* quaternion.
    #[inline]
    pub fn to_matrix(self) -> Matrix3 {
        let q = self.0;
        let ql = q.yzx();
        let qr = q.zxy();

        let q2 = ql * ql + qr * qr;
        let qij = ql * qr;
        let qkr = q.xyz() * q[3];

        let u = qij + qkr;
        let v = qij - qkr;

        let u = u + u;
        let v = v + v;
        let w = Vector3::splat(1.0) - (q2 + q2);

        mat3(
            vec3(w[0], u[2], v[1]),
            vec3(v[2], w[1], u[0]),
            vec3(u[1], v[0], w[2]),
        )
    }

    #[inline(always)]
    pub fn to_mat4(self) -> Matrix4 {
        self.to_matrix().xyz1()
    }
}

impl Mul<Quaternion> for Quaternion {
    type Output = Quaternion;
    #[inline]
    fn mul(self, other: Quaternion) -> Quaternion {
        // TODO: Consider rounding error
        let c = self.0.xyz().cross(other.0.xyz()).into();
        let mut v = other * self[3] + self * other[3] + c;
        v[3] = Quaternion(self.0 * other.0).conjugate().0.sum();
        v
    }
}

impl MulAssign<Quaternion> for Quaternion {
    #[inline(always)]
    fn mul_assign(&mut self, other: Quaternion) {
        *self = *self * other
    }
}

impl Div<Quaternion> for Quaternion {
    type Output = Quaternion;
    #[inline]
    fn div(self, other: Quaternion) -> Quaternion {
        let c = self.0.xyz().cross(other.0.xyz()).into();
        let mut v = self * other[3] - other * self[3] - c;
        v[3] = self.0.dot(other.0);
        v / other.norm_sq()
    }
}

impl DivAssign<Quaternion> for Quaternion {
    #[inline(always)]
    fn div_assign(&mut self, other: Quaternion) {
        *self = *self / other
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accessors() {
        let v = Quaternion::new(1.0, 0.0, 0.0, 0.0);
        assert_eq!(v.x(), 1.0);
        assert_eq!(v.w(), 0.0);
        assert_eq!(v, vec4(1.0, 0.0, 0.0, 0.0).into());
        assert_eq!(Vector4::from(v), vec4(1.0, 0.0, 0.0, 0.0));
        assert_eq!(
            Quaternion::from(vec4(1.0f32, 1.0, 1.0, 1.0)),
            Quaternion::new(1.0f32, 1.0, 1.0, 1.0),
        );
    }

    #[test]
    fn index() {
        let v = Quaternion::new(0.0, 1.0, 2.0, 3.0);
        assert_eq!(v[0], 0.0);
        assert_eq!(v[1], 1.0);
        assert_eq!(v[2], 2.0);
        assert_eq!(v[3], 3.0);
    }

    #[test]
    #[should_panic]
    fn index_out_of_bounds() {
        let v = Quaternion::new(0.0, 1.0, 2.0, 3.0);
        v[4];
    }

    #[test]
    fn from_into() {
        let a = vec4(1.0, 0.0, 0.0, 0.0);
        let b = Quaternion::new(1.0, 0.0, 0.0, 0.0);
        assert_eq!(Quaternion::from(a), b);
        assert_eq!(a, Vector::from(b));
    }

    #[test]
    fn vec_ops() {
        let v = Quaternion::new(1.0f32, 0.0, 0.0, 0.0);
        let u = Quaternion::new(0.0f32, 1.0, 0.0, 0.0);
        assert_eq!(-v, Quaternion::new(-1.0, 0.0, 0.0, 0.0));
        assert_eq!(u + v, Quaternion::new(1.0, 1.0, 0.0, 0.0));
        assert_eq!(u - v, Quaternion::new(-1.0, 1.0, 0.0, 0.0));
        assert_eq!(v - v, Zero::zero());
        assert_eq!(v + v, Quaternion::new(2.0, 0.0, 0.0, 0.0));
    }

    #[test]
    fn scalar_ops() {
        let v = Quaternion::new(1.0, 0.0, 1.0, 0.0);
        assert_eq!(v * 1.0, v);
        assert_eq!(v * 0.0, Zero::zero());
        assert_eq!(v * 2.0, v + v);
        assert_eq!(v * -1.0, -v);

        assert_eq!(-1.0 * v, v * -1.0);
        assert_eq!(0.0 * v, v * 0.0);
        assert_eq!(1.0 * v, v * 1.0);
        assert_eq!(2.0 * v, v * 2.0);

        assert_eq!(v / 1.0, v);
        assert_eq!((v / 0.0).x(), f32::INFINITY);
        assert!((v / 0.0).y().is_nan());
        assert_eq!(v / 2.0, v - v * 0.5);
        assert_eq!(v / -1.0, -v);
    }

    #[test]
    fn quat_ops() {
        let e = Quaternion::one();
        let i = Quaternion::i();
        let j = Quaternion::j();
        let k = Quaternion::k();

        assert_eq!(e.conjugate(), e);
        assert_eq!(i.conjugate(), -i);
        assert_eq!((e + i).conjugate(), e - i);

        assert_eq!((i + j).norm_sq(), 2.0);
        assert_eq!((i + j).norm(), 2.0f32.sqrt());

        assert_eq!(e.inverse(), e);
        assert_eq!(i.inverse(), -i);
        assert_eq!((i + j).inverse(), -(i + j) / 2.0);
        assert_eq!((e + i).inverse(), (e - i) / 2.0);

        assert_eq!(e * i, i);
        assert_eq!(i * j, k);
        assert_eq!(j * i, -k);
        assert_eq!((i + j) * k, i - j);

        let z = Quaternion::zero();
        assert_eq!(z * e, z);
        assert_eq!(i * z, z);

        assert_eq!(e / e, e);
        assert_eq!(i / e, i);
        assert_eq!(i / i, e);
        assert_eq!(i / j, i * j.inverse());
        assert_eq!(i / j, -k);
        assert_eq!(e / (i + j), (i + j).inverse());
        assert_eq!((i + j) / (i + j), e);
        assert_eq!((i - j) / (i + j), -k);

        let mut x = e;
        x *= x;
        assert_eq!(x, e);
        x *= i;
        assert_eq!(x, i);
        x = i;
        x /= -j;
        assert_eq!(x, k);
    }

    #[test]
    fn to_matrix() {
        let e = Quaternion::one();
        let i = Quaternion::i();

        assert_eq!(e.to_matrix(), Matrix::identity());
        assert_eq!(
            i.to_matrix(),
            [
                [1.0,  0.0,  0.0],
                [0.0, -1.0,  0.0],
                [0.0,  0.0, -1.0],
            ].into(),
        );

        // TODO: More tests using approximate equality
    }
}
