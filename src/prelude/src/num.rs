use std::ops::*;

use crate::float::FloatOps;

pub trait Zero {
    fn zero() -> Self;
}

pub fn zero<T: Zero>() -> T {
    T::zero()
}

pub trait One {
    fn one() -> Self;
}

pub fn one<T: One>() -> T {
    T::one()
}

pub trait FromInt: Sized {
    fn from_u64(val: u64) -> Self;
    fn from_i64(val: i64) -> Self;
    fn from_usize(val: usize) -> Self {
        Self::from_u64(val as _)
    }
    fn from_isize(val: isize) -> Self {
        Self::from_i64(val as _)
    }
}

pub trait FromFloat: FromInt {
    fn from_f64(val: f64) -> Self;
    fn from_f32(val: f32) -> Self {
        Self::from_f64(val as _)
    }
}

macro_rules! impl_int {
    ($name:ident) => {
        impl Zero for $name {
            fn zero() -> Self {
                0u8 as _
            }
        }

        impl One for $name {
            fn one() -> Self {
                1u8 as _
            }
        }

        impl FromInt for $name {
            fn from_u64(val: u64) -> Self {
                val as _
            }

            fn from_i64(val: i64) -> Self {
                val as _
            }
        }

        impl FromFloat for $name {
            fn from_f64(val: f64) -> Self {
                val as _
            }
        }
    }
}

impl_int!(u8);
impl_int!(u16);
impl_int!(u32);
impl_int!(u64);
impl_int!(u128);
impl_int!(i8);
impl_int!(i16);
impl_int!(i32);
impl_int!(i64);
impl_int!(i128);
impl_int!(f32);
impl_int!(f64);

macro_rules! impl_num_ops {
    ($(($Op:ident, $OpAssign:ident),)+) => {
        pub trait NumOps = where
            Self: Sized,
            $(
                Self: std::ops::$Op<Self, Output = Self>,
                Self: for<'a> std::ops::$Op<&'a Self, Output = Self>,
                for<'a> &'a Self: std::ops::$Op<Self, Output = Self>,
                for<'a, 'b> &'a Self: std::ops::$Op<&'b Self, Output = Self>,
                Self: std::ops::$OpAssign<Self>,
                Self: for<'a> std::ops::$OpAssign<&'a Self>,
            )*
            ;
    }
}

impl_num_ops! {
    (Add, AddAssign),
    (Sub, SubAssign),
    (Mul, MulAssign),
    (Div, DivAssign),
    (Rem, RemAssign),
}

macro_rules! impl_shift_ops {
    ($($Rhs:ident,)*) => {
        pub trait ShiftOps = where
            Self: Sized,
            $(
            Self: Shl<$Rhs, Output = Self>,
            Self: for<'r> Shl<&'r $Rhs, Output = Self>,
            for<'l> &'l Self: Shl<$Rhs, Output = Self>,
            for<'l, 'r> &'l Self: Shl<&'r $Rhs, Output = Self>,
            Self: ShlAssign<$Rhs>,
            Self: for<'r> ShlAssign<&'r $Rhs>,

            Self: Shr<$Rhs, Output = Self>,
            Self: for<'r> Shr<&'r $Rhs, Output = Self>,
            for<'l> &'l Self: Shr<$Rhs, Output = Self>,
            for<'l, 'r> &'l Self: Shr<&'r $Rhs, Output = Self>,
            Self: ShrAssign<$Rhs>,
            Self: for<'r> ShrAssign<&'r $Rhs>,
            )*
            ;
    }
}

impl_shift_ops! {
    Self,
    u8,
    u16,
    u32,
    u64,
    usize,
    i8,
    i16,
    i32,
    i64,
    isize,
}

macro_rules! impl_bit_ops {
    ($(($Op:ident, $OpAssign:ident),)*) => {
        pub trait BitOps = where
            Self: ShiftOps,
            Self: Not<Output = Self>,
            $(
            Self: $Op<Self, Output = Self>,
            Self: for<'r> $Op<&'r Self, Output = Self>,
            for<'l> &'l Self: $Op<Self, Output = Self>,
            for<'l, 'r> &'l Self: $Op<&'r Self, Output = Self>,
            Self: $OpAssign<Self>,
            Self: for<'r> $OpAssign<&'r Self>,
            )*
            ;
    }
}

impl_bit_ops! {
    (BitAnd, BitAndAssign),
    (BitOr, BitOrAssign),
    (BitXor, BitXorAssign),
}

pub trait Num
    = NumOps
    + Zero
    + One
    + FromInt
    + std::fmt::Debug
    + std::fmt::Display
    + Default
    + PartialEq
    + PartialOrd
    ;

pub trait Signed = Num + std::ops::Neg<Output = Self>;

pub trait Primitive = Num + Copy;

pub trait Integer = Num + BitOps + Eq + Ord;
pub trait PrimInt = Primitive + Integer;

pub trait Float = Signed + FloatOps + FromFloat;
pub trait PrimFloat = Float + Copy;

#[cfg(test)]
mod tests {
    use super::*;

    fn trait_test_inner<F: PrimFloat>() {
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
