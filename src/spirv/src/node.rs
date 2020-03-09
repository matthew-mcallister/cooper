use std::convert::TryFrom;

use crate::*;
use crate::parser::*;

pub type Id = u32;

crate trait Parse: Sized {
    fn parse<'data>(parser: &mut InstructionParser<'data>) -> Result<Self>;
}

/// Variable-width, arbitrary numeric type used by OpSpecConstant. Only
/// sizes up to 64 bits are supported.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
crate struct AnyNumber(crate u64);

crate trait Node: Parse {
    fn id(&self) -> Id;
}

impl Parse for bool {
    fn parse<'data>(parser: &mut InstructionParser<'data>) -> Result<Self> {
        Ok(parser.consume()? != 0)
    }
}

impl Parse for Id {
    fn parse<'data>(parser: &mut InstructionParser<'data>) -> Result<Self> {
        parser.consume()
    }
}

impl Parse for String {
    fn parse<'data>(parser: &mut InstructionParser<'data>) -> Result<Self> {
        parser.parse_string()
    }
}

impl<T: SpirvEnum> Parse for T
    where ShaderParseError: From<<T as TryFrom<u32>>::Error>
{
    fn parse<'data>(parser: &mut InstructionParser<'data>) -> Result<Self> {
        parser.parse_enum()
    }
}

impl<T: Parse> Parse for Vec<T> {
    fn parse<'data>(parser: &mut InstructionParser<'data>) -> Result<Self> {
        parser.parse_many()
    }
}

impl<T: Parse> Parse for Option<T> {
    fn parse<'data>(parser: &mut InstructionParser<'data>) -> Result<Self> {
        parser.parse_option()
    }
}

impl Parse for AnyNumber {
    fn parse<'data>(parser: &mut InstructionParser<'data>) -> Result<Self> {
        let lo = parser.consume()? as u64;
        let hi = parser.consume().unwrap_or(0) as u64;
        Ok(Self((hi << 32) | lo))
    }
}

macro_rules! impl_parseable {
    (
        $name:ident {
            $($member:ident: $type:ty),*$(,)?
        }
    ) => {
        impl_parseable! { $name[$name] { $($member: $type),* } }
    };
    (
        $name:ident[$op_name:ident] {
            $($member:ident: $type:ty),*$(,)?
        }
    ) => {
        #[derive(Debug, Default, Eq, PartialEq)]
        crate struct $name {
            $(crate $member: $type,)*
        }

        impl Parse for $name {
            fn parse<'data>(parser: &mut InstructionParser<'data>) ->
                Result<Self>
            {
                assert_eq!(parser.op(), Some(Op::$op_name));
                Ok($name {
                    $($member: parser.parse()?,)*
                })
            }
        }
    };
}

macro_rules! impl_parseables {
    (
        $(
            $name:ident$([$op_name:ident])? {
                $($member:ident: $type:ty),*$(,)?
            }
        )*
    ) => {
        $(
            impl_parseable! {
                $name$([$op_name])? { $($member: $type),* }
            }
        )*
    }
}

macro_rules! impl_node {
    (
        $name:ident {
            $($member:ident: $type:ty),*$(,)?
        }
    ) => {
        impl_node! { $name[$name] { $($member: $type),* } }
    };
    (
        $name:ident[$op_name:ident] {
            $($member:ident: $type:ty),*$(,)?
        }
    ) => {
        impl_parseable! {
            $name[$op_name] { $($member: $type,)* }
        }

        impl Node for $name {
            fn id(&self) -> Id {
                self.result
            }
        }
    };
}

macro_rules! impl_nodes {
    ($($name:ident$([$op_name:ident])? { $($member:ident: $type:ty,)* })*) => {
        $(impl_node!($name$([$op_name])? { $($member: $type,)* });)*
    }
}

impl_parseables! {
    EntryPoint {
        execution_model: ExecutionModel,
        function: Id,
        name: String,
        interface: Vec<Id>,
    }
    Source {
        language: SourceLanguage,
        version: u32,
        file: Option<Id>,
        source: Option<String>,
    }
}

impl_nodes! {
    SpvString[String] {
        result: Id,
        value: String,
    }
    Variable {
        ty: Id,
        result: Id,
        storage_class: StorageClass,
        initializer: Option<Id>,
    }
    SpecConstantTrue {
        ty: Id,
        result: Id,
    }
    SpecConstantFalse {
        ty: Id,
        result: Id,
    }
    SpecConstant {
        ty: Id,
        result: Id,
        value: AnyNumber,
    }
    SpecConstantComposite {
        ty: Id,
        result: Id,
        constituents: Vec<Id>,
    }
    TypeVoid {
        result: Id,
    }
    TypeBool {
        result: Id,
    }
    TypeInt {
        result: Id,
        width: u32,
        signed: bool,
    }
    TypeFloat {
        result: Id,
        width: u32,
    }
    TypeVector {
        result: Id,
        component_type: Id,
        component_count: u32,
    }
    TypeMatrix {
        result: Id,
        column_type: Id,
        column_count: u32,
    }
    TypeImage {
        result: Id,
        sampled_type: Id,
        dim: Dim,
        depth: u32,
        arrayed: bool,
        multisampled: bool,
        sampled: ImageSampleState,
        format: ImageFormat,
        access_qualifier: Option<AccessQualifier>,
    }
    TypeSampler {
        result: Id,
    }
    TypeSampledImage {
        result: Id,
    }
    TypeArray {
        result: Id,
        elem: Id,
        length: Id,
    }
    TypeRuntimeArray {
        result: Id,
        elem: Id,
    }
    TypeStruct {
        result: Id,
        members: Vec<Id>,
    }
    TypeOpaque {
        result: Id,
    }
    TypePointer {
        result: Id,
        target: Id,
    }
    TypeFunction {
        result: Id,
        ret: Id,
        params: Vec<Id>,
    }
}
