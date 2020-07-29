use rspirv::dr::ModuleHeader;
use spirv_headers as spv;

use super::data;
use super::Module;

pub trait Iter<'m, T: 'm> = ExactSizeIterator<Item = T> + 'm;

macro_rules! indexed_type {
    ($name:ident) => {
        indexed_type!($name, $name);
    };
    ($name:ident, $data_name:ident) => {
        #[derive(Debug)]
        pub struct $name<'m> {
            module: &'m Module,
            index: u32,
            inner: &'m data::$data_name,
        }

        impl<'m> $name<'m> {
            pub fn module(&self) -> &'m Module {
                self.module
            }

            pub fn index(&self) -> u32 {
                self.index
            }

            fn inner(&self) -> &'m data::$data_name {
                self.inner
            }
        }
    };
}

indexed_type!(Variable);
indexed_type!(Uniform);

#[derive(Debug)]
pub struct EntryPoint<'m> {
    module: &'m Module,
    name: &'m str,
    inner: &'m data::EntryPoint,
}

impl Module {
    crate fn new() -> Self {
        Self {
            header: ModuleHeader::new(0),
            entry_points: Default::default(),
            variables: Default::default(),
            uniforms: Default::default(),
            decorations: Default::default(),
        }
    }

    pub fn entry_points(&self) -> impl Iter<'_, EntryPoint<'_>> {
        self.entry_points.iter().map(move |(name, inner)|
            EntryPoint { module: self, name, inner })
    }

    pub fn get_entry_point<'m>(&'m self, name: &impl AsRef<str>) ->
        Option<EntryPoint<'m>>
    {
        let (name, inner) = self.entry_points.get_key_value(name.as_ref())?;
        Some(EntryPoint { module: self, name, inner })
    }

    pub fn get_variable(&self, index: u32) -> Option<Variable<'_>> {
        let inner = self.variables.get(&index)?;
        Some(Variable { module: self, index, inner })
    }

    pub fn uniforms(&self) -> impl Iter<'_, Uniform<'_>> {
        self.uniforms.iter().map(move |(&index, inner)|
            Uniform { module: self, index, inner })
    }

    pub fn get_uniform(&self, index: u32) -> Option<Uniform<'_>> {
        let inner = self.uniforms.get(&index)?;
        Some(Uniform { module: self, index, inner })
    }
}

impl<'m> EntryPoint<'m> {
    pub fn module(&self) -> &'m Module {
        self.module
    }

    pub fn name(&self) -> &str {
        self.name
    }

    fn inner(&self) -> &'m data::EntryPoint {
        self.inner
    }

    pub fn execution_model(&self) -> spv::ExecutionModel {
        self.inner().execution_model
    }

    pub fn inputs(&self) -> impl Iter<'_, Variable<'_>> {
        self.inner().inputs.iter()
            .map(move |&idx| self.module().get_variable(idx).unwrap())
    }

    pub fn outputs(&self) -> impl Iter<'_, Variable<'_>> {
        self.inner().outputs.iter()
            .map(move |&idx| self.module().get_variable(idx).unwrap())
    }
}

impl Variable<'_> {
    pub fn storage_class(&self) -> spv::StorageClass {
        self.inner().storage_class
    }

    pub fn location(&self) -> u32 {
        self.inner().location
    }

    pub fn name(&self) -> Option<&str> {
        Some(&self.inner().name.as_ref()?)
    }
}

impl Uniform<'_> {
    pub fn storage_class(&self) -> spv::StorageClass {
        self.inner().storage_class
    }

    pub fn set(&self) -> u32 {
        self.inner().set
    }

    pub fn binding(&self) -> u32 {
        self.inner().binding
    }

    pub fn name(&self) -> Option<&str> {
        Some(&self.inner().name.as_ref()?)
    }
}
