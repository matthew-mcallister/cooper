use std::convert::TryFrom;

use crate::*;

crate type Id = u32;

crate trait Parse: Sized {
    fn parse<'data>(parser: &mut InstructionParser<'data>) -> Result<Self>;
}

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

macro_rules! impl_parseable {
    (
        $name:ident {
            $($member:ident: $type:ty),*$(,)?
        }
    ) => {
        #[derive(Debug, Eq, PartialEq)]
        crate struct $name {
            $(crate $member: $type,)*
        }

        impl Parse for $name {
            fn parse<'data>(parser: &mut InstructionParser<'data>) ->
                Result<Self>
            {
                assert_eq!(parser.op(), Some(Op::$name));
                Ok($name {
                    $($member: parser.parse()?,)*
                })
            }
        }
    }
}

macro_rules! impl_node {
    (
        $name:ident {
            $($member:ident: $type:ty),*$(,)?
        }
    ) => {
        impl_parseable! {
            $name { $($member: $type,)* }
        }

        impl Node for $name {
            fn id(&self) -> Id {
                self.result
            }
        }
    }
}

macro_rules! impl_nodes {
    ($($name:ident { $($member:ident: $type:ty,)* })*) => {
        $(impl_node!($name { $($member: $type,)* });)*
    }
}

// TODO: EntryPoint, Variable, etc. require public-facing versions.
impl_parseable! {
    EntryPoint {
        execution_model: ExecutionModel,
        function: Id,
        name: String,
        interface: Vec<Id>,
    }
}

impl_nodes! {
    Variable {
        ty: Id,
        result: Id,
        storage_class: StorageClass,
        initializer: Option<Id>,
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
