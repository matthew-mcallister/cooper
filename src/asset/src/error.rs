use std::io;
use std::option::NoneError;

use derive_more::{Display, From, Into};
use image::error::ImageError;

macro_rules! tassert {
    ($cond:expr) => { if !$cond { throw!(); } };
    ($cond:expr, $($args:expr),*$(,)?) => {
        if !$cond { throw!(format!($($args,)*)); }
    };
}

macro_rules! impl_from {
    ($src:ty, $dst:ty, $val:expr) => {
        impl From<$src> for $dst {
            fn from(_: $src) -> Self {
                $val
            }
        }
    }
}

#[derive(Debug, Display, From, Into)]
#[display(fmt = "{}", _0)]
pub struct Error(io::Error);

impl std::error::Error for Error {}

impl Default for Error {
    fn default() -> Self {
        io::ErrorKind::InvalidData.into()
    }
}

impl From<io::ErrorKind> for Error {
    fn from(error: io::ErrorKind) -> Self {
        Self(error.into())
    }
}

// TODO: This pattern could easily be a macro
impl From<ImageError> for Error {
    fn from(error: ImageError) -> Self {
        match error {
            ImageError::IoError(e) => e,
            e => io::Error::new(io::ErrorKind::InvalidData, e),
        }.into()
    }
}

impl From<gltf::Error> for Error {
    fn from(error: gltf::Error) -> Self {
        use gltf::Error;
        match error {
            Error::Io(e) => e,
            e => io::Error::new(io::ErrorKind::InvalidData, e),
        }.into()
    }
}

// TODO: Enable better errors in debug builds
impl_from!(&str, Error, Default::default());
impl_from!(String, Error, Default::default());
impl_from!(NoneError, Error, Default::default());
impl_from!(base64::DecodeError, Error, Default::default());

pub type Result<T> = std::result::Result<T, Error>;
