//! This module defines a trait, `FloatOps`, providing all the floating
//! point operations and constants of the `f32` and `f64` types for
//! writing generic code.

use crate::num::*;

macro_rules! float_ops {
    (
        $(fn $fn:ident(self$(, $arg:ident: $ty:ty)*$(,)?) -> $ret:ty;)*
        $(const $const:ident: $const_ty:ty;)*
    ) => {
        pub trait FloatOps: Sized {
            $(fn $fn(self$(, $arg: $ty)*) -> $ret;)*
            $(const $const: $const_ty;)*
        }

        impl FloatOps for f32 {
            $(
                #[inline(always)]
                fn $fn(self$(, $arg: $ty)*) -> $ret {
                    Self::$fn(self$(, $arg)*)
                }
            )*
            $(const $const: $const_ty = Self::$const;)*
        }

        impl FloatOps for f64 {
            $(
                #[inline(always)]
                fn $fn(self$(, $arg: $ty)*) -> $ret {
                    Self::$fn(self$(, $arg)*)
                }
            )*
            $(const $const: $const_ty = Self::$const;)*
        }
    }
}

float_ops! {
    fn floor(self) -> Self;
    fn ceil(self) -> Self;
    fn round(self) -> Self;
    fn trunc(self) -> Self;
    fn fract(self) -> Self;
    fn abs(self) -> Self;
    fn signum(self) -> Self;
    fn copysign(self, sign: Self) -> Self;
    fn mul_add(self, a: Self, b: Self) -> Self;
    fn powi(self, n: i32) -> Self;
    fn powf(self, n: Self) -> Self;
    fn sqrt(self) -> Self;
    fn exp(self) -> Self;
    fn exp2(self) -> Self;
    fn ln(self) -> Self;
    fn log(self, base: Self) -> Self;
    fn log2(self) -> Self;
    fn log10(self) -> Self;
    fn cbrt(self) -> Self;
    fn hypot(self, other: Self) -> Self;
    fn sin(self) -> Self;
    fn cos(self) -> Self;
    fn tan(self) -> Self;
    fn asin(self) -> Self;
    fn acos(self) -> Self;
    fn atan(self) -> Self;
    fn atan2(self, other: Self) -> Self;
    fn sin_cos(self) -> (Self, Self);
    fn exp_m1(self) -> Self;
    fn ln_1p(self) -> Self;
    fn sinh(self) -> Self;
    fn cosh(self) -> Self;
    fn tanh(self) -> Self;
    fn asinh(self) -> Self;
    fn acosh(self) -> Self;
    fn atanh(self) -> Self;
    fn clamp(self, min: Self, max: Self) -> Self;
    fn is_nan(self) -> bool;
    fn is_infinite(self) -> bool;
    fn is_finite(self) -> bool;
    fn is_normal(self) -> bool;
    fn classify(self) -> std::num::FpCategory;
    fn is_sign_positive(self) -> bool;
    fn is_sign_negative(self) -> bool;
    fn recip(self) -> Self;
    fn to_degrees(self) -> Self;
    fn to_radians(self) -> Self;
    fn max(self, other: Self) -> Self;
    fn min(self, other: Self) -> Self;

    const RADIX: u32;
    const MANTISSA_DIGITS: u32;
    const DIGITS: u32;
    const EPSILON: Self;
    const MIN: Self;
    const MIN_POSITIVE: Self;
    const MAX: Self;
    const MIN_EXP: i32;
    const MAX_EXP: i32;
    const MIN_10_EXP: i32;
    const MAX_10_EXP: i32;
    const NAN: Self;
    const INFINITY: Self;
    const NEG_INFINITY: Self;
}

pub trait FromFloat: FromInt {
    fn from_f64(val: f64) -> Self;

    #[inline(always)]
    fn from_f32(val: f32) -> Self {
        Self::from_f64(val as _)
    }
}

pub trait Float = Signed + FloatOps + FromFloat;
pub trait PrimFloat = Float + Copy;

#[cfg(test)]
mod tests {
    use super::*;

    fn trait_test_inner<F: Signed + FloatOps + FromFloat + Copy>()
    {
        let a = F::from_f32(1.0);
        let b = F::from_f32(2.5);
        assert_eq!(a * b, b);
        assert_eq!(b.floor(), F::from_f32(2.0));
        assert_eq!(F::RADIX, 2);
        assert_eq!(F::zero().clamp(a, b), a);
    }

    #[test]
    fn trait_test_f32() {
        trait_test_inner::<f32>();
    }

    #[test]
    fn trait_test_f64() {
        trait_test_inner::<f64>();
    }
}
