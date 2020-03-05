// This module is 90% copypasta

#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]
#![allow(path_statements)]

use std::convert::TryFrom;
use std::ops::*;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct InvalidEnumValue(pub u32);
impl std::fmt::Display for InvalidEnumValue {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "invalid enum value: {}", self.0)
    }
}

macro_rules! impl_unary_op {
    ($OpName:ident, $opname:ident; $name:ident) => {
        impl $OpName for $name {
            type Output = Self;
            #[inline]
            fn $opname(self) -> Self {
                $name((self.0).$opname())
            }
        }
    }
}

macro_rules! impl_bin_op {
    ($OpName:ident, $opname:ident; $name:ident) => {
        impl $OpName for $name {
            type Output = Self;
            #[inline]
            fn $opname(self, other: Self) -> Self {
                $name((self.0).$opname(other.0))
            }
        }
    }
}

macro_rules! impl_bin_op_assign {
    ($OpAssign:ident, $opassign:ident; $name:ident) => {
        impl $OpAssign for $name {
            #[inline]
            fn $opassign(&mut self, other: Self) {
                (self.0).$opassign(other.0)
            }
        }
    }
}

macro_rules! impl_enum {
    (
        Value $name:ident {
            $($member:ident = $value:expr,)*
            $(+$alias:ident = $alias_val:ident,)*
        }
    ) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        pub enum $name {
            $($member = $value,)*
        }

        impl $name {
            $(pub const $alias: Self = Self::$alias_val;)*
        }

        impl Default for $name {
            fn default() -> Self {
                // First member is the default
                let res = $(Self::$member;)*
                res
            }
        }

        impl From<$name> for u32 {
            fn from(val: $name) -> Self {
                val as _
            }
        }

        impl TryFrom<u32> for $name {
            type Error = InvalidEnumValue;
            fn try_from(val: u32) -> Result<Self, Self::Error> {
                match val {
                    $($value => Ok(Self::$member),)*
                    _ => Err(InvalidEnumValue(val)),
                }
            }
        }
    };
    (
        Bit $name:ident {
            $($member:ident = $bit:expr,)*
            $(+$alias:ident = $alias_val:ident,)*
        }
    ) => {
        #[derive(Clone, Copy, Default, Eq, Hash, PartialEq)]
        pub struct $name(pub u32);

        impl $name {
            $(pub const $member: Self = Self(1u32 << $bit);)*
            $(pub const $alias: Self = Self::$alias_val;)*

            #[inline]
            pub fn empty() -> Self { $name(0) }
            #[inline]
            pub fn is_empty(self) -> bool { self.0 == 0 }
            #[inline]
            pub fn intersects(self, other: Self) -> bool
                { self.bitand(other).0 != 0 }
            #[inline]
            pub fn contains(self, other: Self) -> bool
                { self.bitand(other).0 == other.0 }
        }

        impl_unary_op!(Not, not; $name);
        impl_bin_op!(BitAnd, bitand; $name);
        impl_bin_op_assign!(BitAndAssign, bitand_assign; $name);
        impl_bin_op!(BitOr, bitor; $name);
        impl_bin_op_assign!(BitOrAssign, bitor_assign; $name);
        impl_bin_op!(BitXor, bitxor; $name);
        impl_bin_op_assign!(BitXorAssign, bitxor_assign; $name);

        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.debug_tuple(stringify!($name))
                    .field(&format_args!("0x{:x}", self.0))
                    .finish()
            }
        }
    };
}

macro_rules! impl_enums {
    ($($type:ident $name:ident $body:tt)*) => {
        $(impl_enum! { $type $name $body })*
    }
}

include!(concat!(env!("CARGO_MANIFEST_DIR"), "/generated/generated.rs"));
