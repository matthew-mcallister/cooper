use derive_more::*;

#[derive(Clone, Constructor, Copy, Debug, Display)]
#[display(fmt = "unrecognized enum value: {}", value)]
pub struct EnumValueError {
    value: u32,
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
