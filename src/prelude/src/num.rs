use crate::float::*;

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

//macro_rules! float_ops {
//    (
//        $(fn $fn:ident(self) -> Self;)*
//        $(const $const:ident: $const_ty:ty;)*
//    ) => {
//        trait FloatOps: Sized {
//            $(fn $fn(self) -> Self;)*
//            $(const $const: $const_ty;)*
//        }
//        impl FloatOps for f32 {
//            $(fn $fn(self) -> Self { f32::$fn(self) })*
//            $(const $const: $const_ty = f32::$const;)*
//        }
//    }
//}
//

// TODO: Actually want *all 4* variants of binary ops.
pub trait NumOps
    = Sized
    + std::ops::Add<Output = Self>
    + std::ops::Sub<Output = Self>
    + std::ops::Div<Output = Self>
    + std::ops::Mul<Output = Self>
    + std::ops::Rem<Output = Self>
    + std::ops::AddAssign
    + std::ops::SubAssign
    + std::ops::DivAssign
    + std::ops::MulAssign
    + std::ops::RemAssign
    + for<'a> std::ops::AddAssign<&'a Self>
    + for<'a> std::ops::SubAssign<&'a Self>
    + for<'a> std::ops::DivAssign<&'a Self>
    + for<'a> std::ops::MulAssign<&'a Self>
    + for<'a> std::ops::RemAssign<&'a Self>;

pub trait BitOps
    = Sized
    + std::ops::Not<Output = Self>
    + std::ops::BitAnd<Output = Self>
    + std::ops::BitOr<Output = Self>
    + std::ops::BitXor<Output = Self>
    + std::ops::Shl<Output = Self>
    + std::ops::Shr<Output = Self>
    + std::ops::BitAndAssign
    + std::ops::BitOrAssign
    + std::ops::BitXorAssign
    + std::ops::ShlAssign
    + std::ops::ShrAssign
    + for<'a> std::ops::BitAndAssign<&'a Self>
    + for<'a> std::ops::BitOrAssign<&'a Self>
    + for<'a> std::ops::BitXorAssign<&'a Self>
    + for<'a> std::ops::ShlAssign<&'a Self>
    + for<'a> std::ops::ShrAssign<&'a Self>;

pub trait Num
    = NumOps
    + Zero
    + One
    + FromInt
    + std::fmt::Debug
    + std::fmt::Display
    + Default
    + PartialEq
    + PartialOrd;

pub trait Signed = Num + std::ops::Neg<Output = Self>;

pub trait Primitive = Copy + Num;

pub trait Integer = BitOps + Eq + Ord;
pub trait PrimInt = Primitive + Integer;

pub trait Float = FloatOps + FromFloat + Signed;
pub trait PrimFloat = Copy + Float;

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
