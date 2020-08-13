use std::io;
use std::option::NoneError;

use derivative::Derivative;
use derive_more::Display;

macro_rules! tassert {
    ($cond:expr) => { if !$cond { throw!(); } };
    ($cond:expr, $($args:expr),*$(,)?) => {
        if !$cond { throw!(format!($($args,)*)); }
    };
}

// Detailed/debug error type with backtraces.
#[derive(Derivative, Display)]
#[display(fmt = "failed to load asset: {}", _0)]
#[derivative(Debug = "transparent")]
pub struct Error(anyhow::Error);

pub type Result<T> = std::result::Result<T, Error>;

impl std::error::Error for Error {
    #[inline(always)]
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.chain().next()
    }

    #[inline(always)]
    fn backtrace(&self) -> Option<&std::backtrace::Backtrace> {
        Some(self.0.backtrace())
    }
}

impl Default for Error {
    #[inline(always)]
    fn default() -> Self {
        io::ErrorKind::InvalidData.into()
    }
}

impl From<&'static str> for Error {
    #[inline(always)]
    fn from(msg: &'static str) -> Self {
        Self(anyhow::Error::msg(msg))
    }
}

impl From<String> for Error {
    #[inline(always)]
    fn from(msg: String) -> Self {
        Self(anyhow::Error::msg(msg))
    }
}

impl From<NoneError> for Error {
    #[inline(always)]
    fn from(_: NoneError) -> Self {
        Default::default()
    }
}

macro_rules! impl_from {
    ($ty:ty) => {
        impl From<$ty> for Error {
            #[inline(always)]
            fn from(error: $ty) -> Self {
                Self(anyhow::Error::new(error))
            }
        }
    }
}

impl_from!(base64::DecodeError);
impl_from!(gltf::Error);
impl_from!(image::ImageError);
impl_from!(io::Error);

impl From<io::ErrorKind> for Error {
    #[inline(always)]
    fn from(kind: io::ErrorKind) -> Self {
        io::Error::from(kind).into()
    }
}

impl From<Error> for io::Error {
    #[inline(always)]
    fn from(e: Error) -> Self {
        match e.0.downcast::<Self>() {
            Ok(e) => e,
            Err(e) => Self::new(io::ErrorKind::InvalidData, e),
        }
    }
}
