use std::fmt;

use derive_more::*;

#[derive(Clone, Constructor, Copy, Debug)]
pub struct EnumValueError {
    value: u32,
}

impl fmt::Display for EnumValueError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "unrecognized enum value: {}", self.value)
    }
}

impl std::error::Error for EnumValueError {}

#[macro_export]
macro_rules! impl_enum {
    ($name:ident[$type:ident] { $($member:ident = $value:expr,)* }) => {
        #[repr($type)]
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub enum $name {
            $($member = $value,)*
        }
        impl $name {
            const VALUES: &'static [$name] = &[$($name::$member,)*];
        }
        impl std::convert::TryFrom<$type> for $name {
            type Error = EnumValueError;
            fn try_from(value: $type) -> std::result::Result<Self, Self::Error>
            {
                if $name::VALUES.iter().any(|&e| e as $type == value) {
                    Ok(unsafe { std::mem::transmute(value) })
                } else {
                    Err(EnumValueError::new(value as _))
                }
            }
        }
    }
}

#[macro_export]
macro_rules! impl_default {
    ($name:ident, $val:expr) => {
        impl std::default::Default for $name {
            fn default() -> Self {
                $val
            }
        }
    }
}
