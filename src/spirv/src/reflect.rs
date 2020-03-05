use std::convert::{TryFrom, TryInto};

use derive_more::Display;

use crate::*;

pub type Id = u32;

#[derive(Clone, Copy, Debug, Display, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum ShaderParseError {
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

pub type Error = ShaderParseError;
pub type Result<T> = std::result::Result<T, Error>;

impl std::error::Error for Error {}

impl From<InvalidEnumValue> for Error {
    fn from(_: InvalidEnumValue) -> Self {
        Self::InvalidModule
    }
}

impl From<std::str::Utf8Error> for Error {
    fn from(_: std::str::Utf8Error) -> Self {
        Self::InvalidModule
    }
}

pub type Version = (u8, u8);

/// Reflection info for a shader module.
#[derive(Debug, Default)]
pub struct ShaderModule {
    /// The SPIR-V version as a pair `(major, minor)`.
    pub version: Version,
    pub entry_points: Vec<EntryPoint>,
    pub source_language: Option<SourceLanguage>,
    pub source_language_version: u32,
    pub source_file: Option<String>,
    pub source_source: Option<String>,
}

#[derive(Debug)]
pub struct EntryPoint {
    pub execution_model: ExecutionModel,
    pub function: Id,
    pub name: String,
    pub interface: Vec<Id>,
}

impl ShaderModule {
    pub fn new(data: &[u32]) -> Result<Self> {
        let mut parser = ShaderParser::new(data);
        parser.parse_module()?;
        Ok(parser.module)
    }
}

#[derive(Debug)]
struct ShaderParser<'data> {
    module: ShaderModule,
    data: &'data [u32],
}

#[derive(Debug)]
struct InstructionParser<'data> {
    op: Option<Op>,
    // Remaining operand words including the type and result ids
    operands: &'data [u32],
}

// Returns `Ok(None)` upon encountering an unrecognized opcode.
fn decode_op(word: u32) -> (Option<Op>, usize) {
    let size = (word >> 16) & 0xffff;
    let op = (word & 0xffff).try_into().ok();
    (op, size as _)
}

const HEADER_LEN: usize = 5;

fn parse_header(header: &[u32]) -> Result<Version> {
    assert_eq!(header.len(), HEADER_LEN);

    let magic = header[0];
    if magic != 0x07230203 { return Err(Error::InvalidModule); }

    let byte = |word, n| ((word >> n) & 0xffu32) as u8;
    let to_version = |word| (byte(word, 2), byte(word, 1));
    let version = to_version(header[1]);

    Ok(version)
}

impl<'data> InstructionParser<'data> {
    fn bytes(&self) -> &'data [u8] {
        unsafe {
            std::slice::from_raw_parts(
                self.operands.as_ptr() as _,
                self.operands.len() * std::mem::size_of::<u32>(),
            )
        }
    }

    fn consume(&mut self) -> Result<u32> {
        let (&first, rest) = self.operands.split_first()
            .ok_or(Error::InvalidModule)?;
        self.operands = rest;
        Ok(first)
    }

    fn parse_enum<T>(&mut self) -> Result<T>
    where
        T: TryFrom<u32>,
        Error: From<T::Error>,
    {
        Ok(self.consume()?.try_into()?)
    }

    fn parse_string(&mut self) -> Result<String> {
        assert!(cfg!(target_endian = "little"));
        let bytes = &self.bytes();
        let len = bytes.iter().position(|&b| b == 0)
            .ok_or(Error::InvalidModule)?;
        let s = std::str::from_utf8(&bytes[..len])?;

        let word_size = std::mem::size_of::<u32>();
        let word_count = (s.len() + word_size) / word_size;
        self.operands = &self.operands[word_count..];

        Ok(s.to_owned())
    }

    fn parse_entry_point(mut self) -> Result<EntryPoint> {
        assert_eq!(self.op, Some(Op::EntryPoint));
        let execution_model = self.parse_enum()?;
        let function = self.consume()?;
        let name = self.parse_string()?;
        let interface = self.operands.to_owned();
        Ok(EntryPoint {
            execution_model,
            function,
            name,
            interface,
        })
    }
}

impl<'data> ShaderParser<'data> {
    fn new(data: &'data [u32]) -> Self {
        ShaderParser {
            module: Default::default(),
            data,
        }
    }

    fn insts(&self) -> impl Iterator<Item = Result<InstructionParser<'data>>> {
        #[derive(Debug)]
        struct InstIter<'a> {
            data: &'a [u32],
        }

        impl<'a> Iterator for InstIter<'a> {
            type Item = Result<InstructionParser<'a>>;
            fn next(&mut self) -> Option<Self::Item> {
                let (op, words) = decode_op(*self.data.first()?);
                if self.data.len() < words {
                    self.data = &[];
                    return Some(Err(Error::InvalidModule));
                }

                let (operands, data) = self.data.split_at(words);
                self.data = data;
                Some(Ok(InstructionParser {
                    op,
                    operands: &operands[1..],
                }))
            }
        }

        InstIter {
            data: &self.data[HEADER_LEN..],
        }
    }

    fn parse_module(&mut self) -> Result<()> {
        self.parse_header()?;
        self.parse_entry_points()?;
        Ok(())
    }

    fn parse_header(&mut self) -> Result<()> {
        let header = self.data.get(..HEADER_LEN).ok_or(Error::InvalidModule)?;
        self.module.version = parse_header(header)?;
        Ok(())
    }

    fn parse_entry_points(&mut self) -> Result<()> {
        for inst in self.insts() {
            let inst = inst?;
            if inst.op != Some(Op::EntryPoint) { continue; }
            self.module.entry_points.push(inst.parse_entry_point()?);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE_SPV: &'static [u8] = include_bytes!(
        concat!(env!("CARGO_MANIFEST_DIR"), "/build/example.spv"));

    fn example_spv() -> &'static [u32] {
        let word_size = std::mem::size_of::<u32>();
        assert_eq!(EXAMPLE_SPV.len() % word_size, 0);
        assert_eq!(EXAMPLE_SPV.as_ptr() as usize % word_size, 0);
        unsafe {
            std::slice::from_raw_parts(
                EXAMPLE_SPV.as_ptr() as *const u32,
                EXAMPLE_SPV.len() / word_size,
            )
        }
    }

    #[test]
    fn test_example() {
        let module = ShaderModule::new(example_spv()).unwrap();
        assert_eq!(module.entry_points.len(), 1);
        let entry_point = &module.entry_points[0];
        assert_eq!(entry_point.execution_model, ExecutionModel::Vertex);
        assert_eq!(entry_point.name, "main");
    }
}
