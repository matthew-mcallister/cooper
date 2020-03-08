use std::convert::{TryFrom, TryInto};

use fnv::FnvHashMap;

use crate::*;

macro_rules! impl_nodes {
    (
        $($op:ident,)*
        $(@box $box_op:ident,)*
    ) => {
        #[derive(Debug, Eq, PartialEq)]
        crate enum AnyNode {
            $($op($op),)*
            $($box_op(Box<$box_op>),)*
        }

        impl AnyNode {
            crate fn parse<'data>(inst: &mut InstructionParser<'data>) ->
                Result<Option<Self>>
            {
                Ok(match inst.op {
                    $(Some(Op::$op) =>
                        Some(Self::from(inst.parse()?: $op)),)*
                    $(Some(Op::$box_op) =>
                        Some(Self::from(inst.parse()?: $box_op)),)*
                    _ => None,
                })
            }

            crate fn id(&self) -> Id {
                match self {
                    $(Self::$op(ref val) => val.id(),)*
                    $(Self::$box_op(ref val) => val.id(),)*
                }
            }
        }

        $(impl From<$op> for AnyNode {
            fn from(val: $op) -> Self {
                Self::$op(val)
            }
        })*

        $(impl From<$box_op> for AnyNode {
            fn from(val: $box_op) -> Self {
                Self::$box_op(Box::new(val))
            }
        })*
    }
}

impl_nodes! {
    Variable,
    TypeVoid,
    TypeBool,
    TypeInt,
    TypeFloat,
    TypeVector,
    TypeMatrix,
    TypeSampler,
    TypeSampledImage,
    TypeArray,
    TypeRuntimeArray,
    TypeStruct,
    TypeOpaque,
    TypePointer,
    TypeFunction,
    @box TypeImage,
}

#[derive(Debug)]
crate struct ShaderParser<'data> {
    crate version: Version,
    crate nodes: FnvHashMap<Id, AnyNode>,
    crate data: &'data [u32],
}

#[derive(Debug)]
crate struct InstructionParser<'data> {
    op: Option<Op>,
    // Remaining operand words including the type and result ids
    operands: &'data [u32],
}

// Returns `(None, _)` upon encountering an unrecognized opcode.
fn decode_op(word: u32) -> (Option<Op>, usize) {
    let size = (word >> 16) & 0xffff;
    let op = (word & 0xffff).try_into().ok();
    (op, size as _)
}

const HEADER_LEN: usize = 5;

fn parse_header(header: &[u32]) -> Result<Version> {
    assert_eq!(header.len(), HEADER_LEN);

    let magic = header[0];
    if magic != 0x07230203 { Err(ErrorKind::InvalidModule)?; }

    let byte = |word, n| ((word >> (8 * n)) & 0xffu32) as u8;
    let to_version = |word| (byte(word, 2), byte(word, 1));
    let version = to_version(header[1]);

    Ok(version)
}

impl<'data> InstructionParser<'data> {
    crate fn op(&self) -> Option<Op> {
        self.op
    }

    crate fn bytes(&self) -> &'data [u8] {
        unsafe {
            std::slice::from_raw_parts(
                self.operands.as_ptr() as _,
                self.operands.len() * std::mem::size_of::<u32>(),
            )
        }
    }

    crate fn consume(&mut self) -> Result<u32> {
        let (&first, rest) = self.operands.split_first()
            .ok_or(ErrorKind::InvalidModule)?;
        self.operands = rest;
        Ok(first)
    }

    crate fn parse_string(&mut self) -> Result<String> {
        assert!(cfg!(target_endian = "little"));
        let bytes = &self.bytes();
        let len = bytes.iter().position(|&b| b == 0)
            .ok_or(ErrorKind::InvalidModule)?;
        let s = std::str::from_utf8(&bytes[..len])?;

        let word_size = std::mem::size_of::<u32>();
        let word_count = (s.len() + word_size) / word_size;
        self.operands = &self.operands[word_count..];

        Ok(s.to_owned())
    }

    crate fn parse_enum<T: SpirvEnum>(&mut self) -> Result<T>
        where ShaderParseError: From<<T as TryFrom<u32>>::Error>
    {
        Ok(self.consume()?.try_into()?)
    }

    crate fn parse<T: Parse>(&mut self) -> Result<T> {
        T::parse(self)
    }

    crate fn parse_many<T: Parse>(&mut self) -> Result<Vec<T>> {
        let mut res = Vec::new();
        while !self.operands.is_empty() {
            res.push(self.parse()?);
        }
        Ok(res)
    }

    crate fn parse_option<T: Parse>(&mut self) -> Result<Option<T>> {
        Ok(if !self.operands.is_empty() {
            Some(self.parse()?)
        } else { None })
    }
}

impl<'data> ShaderParser<'data> {
    crate fn new(data: &'data [u32]) -> Self {
        ShaderParser {
            version: Default::default(),
            nodes: Default::default(),
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
                    return Some(Err(ErrorKind::InvalidModule.into()));
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

    fn parse_header(&mut self) -> Result<()> {
        let header = self.data.get(..HEADER_LEN)
            .ok_or(ErrorKind::InvalidModule)?;
        self.version = parse_header(header)?;
        Ok(())
    }

    fn parse_nodes(&mut self) -> Result<()> {
        for inst in self.insts() {
            if let Some(node) = AnyNode::parse(&mut inst?)? {
                assert!(!self.nodes.contains_key(&node.id()));
                self.nodes.insert(node.id(), node);
            }
        }
        Ok(())
    }

    crate fn parse_module(&mut self) -> Result<()> {
        self.parse_header()?;
        self.parse_nodes()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_test() {
        let example = testing::example_spv();
        let mut parser = ShaderParser::new(&example);
        parser.parse_module().unwrap();
        assert_eq!(parser.version, (1, 0));
        assert_eq!(
            &parser.nodes[&3],
            &AnyNode::TypeVoid(TypeVoid { result: 3 }),
        );
        assert_eq!(
            &parser.nodes[&9],
            &AnyNode::TypeMatrix(TypeMatrix {
                result: 9,
                column_type: 8,
                column_count: 4,
            }),
        );
        assert_eq!(
            &parser.nodes[&14],
            &AnyNode::Variable(Variable {
                ty: 13,
                result: 14,
                storage_class: StorageClass::Input,
                initializer: None,
            }),
        );
    }
}
