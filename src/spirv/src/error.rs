#[cfg(test)]
use std::backtrace::Backtrace;

use derive_more::*;

use crate::*;

#[derive(Clone, Copy, Debug, Display, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum ShaderParseErrorKind {
    /// The byte stream is not a valid SPIR-V module.
    #[display(fmt = "invalid module")]
    InvalidModule,
    /// A library limitation has been exceeded, such as an array length.
    #[display(fmt = "limit exceeded")]
    LimitExceeded,
    /// This parser is incapable of parsing the current module.
    #[display(fmt = "unsupported module")]
    UnsupportedModule,
}

#[cfg(not(test))]
#[derive(Debug, Display)]
#[display(fmt = "{}", kind)]
pub struct ShaderParseError {
    kind: ErrorKind,
}

#[cfg(test)]
#[derive(Debug, Display)]
#[display(fmt = "{}\n{}", kind, backtrace)]
pub struct ShaderParseError {
    kind: ErrorKind,
    backtrace: Backtrace,
}

pub type ErrorKind = ShaderParseErrorKind;
pub type Error = ShaderParseError;
pub type Result<T> = std::result::Result<T, Error>;

impl std::error::Error for Error {
    #[cfg(test)]
    fn backtrace(&self) -> Option<&Backtrace> {
        Some(&self.backtrace)
    }
}

impl Error {
    #[cfg(test)]
    fn new(kind: ErrorKind) -> Self {
        Self {
            kind,
            backtrace: Backtrace::capture(),
        }
    }

    #[cfg(not(test))]
    fn new(kind: ErrorKind) -> Self {
        Self { kind }
    }

    fn kind(&self) -> ErrorKind {
        self.kind
    }
}

impl From<InvalidEnumValue> for ErrorKind {
    fn from(_: InvalidEnumValue) -> Self {
        Self::InvalidModule
    }
}

impl From<std::str::Utf8Error> for ErrorKind {
    fn from(_: std::str::Utf8Error) -> Self {
        Self::InvalidModule
    }
}

impl<T> From<T> for Error
    where ErrorKind: From<T>
{
    fn from(val: T) -> Self {
        Self::new(val.into())
    }
}
