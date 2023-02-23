use rspirv::{self, dr};
use rspirv::binary::{Consumer, ParseAction};
use spirv_headers as spv;

use super::is_interface_storage;
use super::data;
use super::data::Module;

#[derive(Debug)]
struct RawModule {
    header: dr::ModuleHeader,
    instructions: Vec<dr::Instruction>,
}

impl RawModule {
    fn new() -> Self {
        Self {
            header: dr::ModuleHeader::new(0),
            instructions: Default::default(),
        }
    }

    #[inline]
    fn occurrences(&self, opcode: spv::Op) ->
        impl Iterator<Item = &'_ dr::Instruction> + '_
    {
        self.instructions.iter()
            .filter(move |inst| inst.class.opcode == opcode)
    }
}

macro_rules! get_operand_variant {
    ($operand:expr, $variant:ident) => {
        match $operand {
            dr::Operand::$variant(ref val) => val.clone(),
            _ => panic!(concat!("expected ", stringify!($variant))),
        }
    }
}

macro_rules! parse_operand {
    ($operands:expr, $variant:ident) => {
        get_operand_variant!($operands.next().unwrap(), $variant)
    };
    ($operands:expr, $variant:ident*) => {
        $operands.map(|operand| get_operand_variant!(operand, $variant))
            .collect::<Vec<_>>()
    };
}

fn raise_module(raw: &RawModule) -> Module {
    let mut module = Module::new();
    build_decoration_sets(&mut module, raw);
    raise_variables(&mut module, raw);
    raise_entry_points(&mut module, raw);
    module.decorations = Default::default(); // No longer needed
    module
}

fn build_decoration_sets(module: &mut Module, raw: &RawModule) {
    for inst in raw.instructions.iter() {
        let operands = &inst.operands;
        match inst.class.opcode {
            spv::Op::Decorate => apply_decoration(module, operands),
            spv::Op::Name => apply_name(module, operands),
            _ => {},
        }
    }
}

fn apply_decoration(module: &mut Module, operands: &[dr::Operand]) {
    let mut ops = operands.iter();
    let target = parse_operand!(ops, IdRef);
    let decoration = parse_operand!(ops, Decoration);
    let mut decos = module.decorations.entry(target).or_default();
    match decoration {
        spv::Decoration::Location => {
            let val = parse_operand!(ops, LiteralInt32);
            decos.location = Some(val);
        },
        spv::Decoration::Binding => {
            let val = parse_operand!(ops, LiteralInt32);
            decos.binding = Some(val);
        },
        spv::Decoration::DescriptorSet => {
            let val = parse_operand!(ops, LiteralInt32);
            decos.set = Some(val);
        },
        _ => {},
    }
}

fn apply_name(module: &mut Module, operands: &[dr::Operand]) {
    let mut ops = operands.iter();
    let target = parse_operand!(ops, IdRef);
    let name = parse_operand!(ops, LiteralString);
    let mut decos = module.decorations.entry(target).or_default();
    decos.name = Some(name);
}

fn raise_variables(module: &mut Module, raw: &RawModule) {
    for inst in raw.occurrences(spv::Op::Variable) {
        raise_variable(module, inst);
    }
}

fn raise_variable(module: &mut Module, inst: &dr::Instruction) {
    assert_eq!(inst.class.opcode, spv::Op::Variable);
    let mut ops = inst.operands.iter();
    let id = inst.result_id.unwrap();

    let storage_class = parse_operand!(ops, StorageClass);
    if storage_class == spv::StorageClass::Function { return; }

    let decos = module.decorations.entry(id).or_default();
    match (decos.location, decos.set, decos.binding) {
        (Some(location), _, _) => {
            assert!(is_interface_storage(storage_class));
            module.variables.insert(id, data::Variable {
                storage_class,
                location,
                name: decos.name.clone(),
            });
        },
        (_, Some(set), Some(binding)) => {
            assert!(!is_interface_storage(storage_class));
            module.uniforms.insert(id, data::Uniform {
                storage_class,
                set,
                binding,
                name: decos.name.clone(),
            });
        },
        _ => {},
    }
}

fn raise_entry_points(module: &mut Module, raw: &RawModule) {
    for inst in raw.occurrences(spv::Op::EntryPoint) {
        raise_entry_point(module, inst);
    }
}

fn raise_entry_point(module: &mut Module, inst: &dr::Instruction) {
    assert_eq!(inst.class.opcode, spv::Op::EntryPoint);

    let mut ops = inst.operands.iter();
    let execution_model = parse_operand!(ops, ExecutionModel);
    let _function = parse_operand!(ops, IdRef);
    let name = parse_operand!(ops, LiteralString);
    let interface = parse_operand!(ops, IdRef*);

    let mut inputs = Vec::new();
    let mut outputs = Vec::new();
    for (idx, var) in interface.iter().copied()
        .filter_map(|idx| Some((idx, module.variables.get(&idx)?)))
    {
        match var.storage_class {
            spv::StorageClass::Input => inputs.push(idx),
            spv::StorageClass::Output => outputs.push(idx),
            _ => unreachable!(),
        }
    }

    module.entry_points.insert(name, data::EntryPoint {
        execution_model,
        inputs,
        outputs,
    });
}

impl Consumer for RawModule {
    fn initialize(&mut self) -> ParseAction {
        ParseAction::Continue
    }

    fn finalize(&mut self) -> ParseAction {
        ParseAction::Continue
    }

    fn consume_header(&mut self, header: dr::ModuleHeader) -> ParseAction {
        self.instructions.reserve(header.bound as usize);
        self.header = header;
        ParseAction::Continue
    }

    fn consume_instruction(&mut self, inst: dr::Instruction) -> ParseAction {
        self.instructions.push(inst);
        ParseAction::Continue
    }
}

pub fn parse_words(words: &impl AsRef<[u32]>) -> Module {
    let mut raw = RawModule::new();
    rspirv::binary::parse_words(words, &mut raw).unwrap();
    raise_module(&raw)
}

pub fn parse_bytes(bytes: &impl AsRef<[u8]>) -> Module {
    let mut raw = RawModule::new();
    rspirv::binary::parse_bytes(bytes, &mut raw).unwrap();
    raise_module(&raw)
}
