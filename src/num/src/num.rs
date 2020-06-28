use std::ops::*;

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

macro_rules! impl_int {
    ($name:ident) => {
        impl Zero for $name {
            #[inline(always)]
            fn zero() -> Self {
                0u8 as _
            }
        }

        impl One for $name {
            #[inline(always)]
            fn one() -> Self {
                1u8 as _
            }
        }

        impl FromInt for $name {
            #[inline(always)]
            fn from_u64(val: u64) -> Self {
                val as _
            }

            #[inline(always)]
            fn from_i64(val: i64) -> Self {
                val as _
            }
        }

        impl crate::float::FromFloat for $name {
            #[inline(always)]
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
                Self: $Op<Self, Output = Self>,
                Self: for<'a> $Op<&'a Self, Output = Self>,
                for<'a> &'a Self: $Op<Self, Output = Self>,
                for<'a, 'b> &'a Self: $Op<&'b Self, Output = Self>,
                Self: $OpAssign<Self>,
                Self: for<'a> $OpAssign<&'a Self>,
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
            for<'a> &'a Self: Not<Output = Self>,
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
    + std::iter::Sum
    + std::fmt::Debug
    + std::fmt::Display
    + Default
    + PartialEq
    + PartialOrd
    ;

pub trait Signed = where
    Self: Num,
    Self: Neg<Output = Self>,
    for<'a> &'a Self: Neg<Output = Self>,
    ;

pub trait Primitive = Num + Copy;

pub trait Integer = Num + BitOps + Eq + Ord;
pub trait PrimInt = Primitive + Integer;

pub trait PrimSigned = Primitive + Signed;
