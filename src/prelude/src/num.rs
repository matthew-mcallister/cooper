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

macro_rules! impl_primitive {
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
    }
}

impl_primitive!(u8);
impl_primitive!(u16);
impl_primitive!(u32);
impl_primitive!(u64);
impl_primitive!(u128);
impl_primitive!(i8);
impl_primitive!(i16);
impl_primitive!(i32);
impl_primitive!(i64);
impl_primitive!(i128);
impl_primitive!(f32);
impl_primitive!(f64);

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
    + std::ops::RemAssign;

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
    + std::ops::ShrAssign;

pub trait Num
    = NumOps
    + Zero
    + One
    + std::fmt::Debug
    + std::fmt::Display
    + Default
    + PartialEq
    + PartialOrd
    + std::hash::Hash;

pub trait Signed = Num + std::ops::Neg<Output = Self>;

pub trait Primitive = Copy + Num;

pub trait Integer = BitOps + Eq + Ord;

pub trait PrimInt = Primitive + Integer;
