use cooper_spirv::*;
use spirv_headers as spv;

#[test]
fn parse() {
    let data = std::fs::read("data/static_vert.spv").unwrap();
    let module = parse_bytes(&data);

    assert_eq!(module.entry_points().len(), 1);

    let entry = module.get_entry_point(&"main").unwrap();
    assert_eq!(entry.execution_model(), spv::ExecutionModel::Vertex);
    assert_eq!(entry.inputs().len(), 3);
    assert_eq!(entry.outputs().len(), 2);

    let tex_coord = entry.inputs().find(|var| var.location() == 4).unwrap();
    assert_eq!(tex_coord.storage_class(), spv::StorageClass::Input);
    // TODO: Use a shader where the debug info is never stripped
    assert_eq!(tex_coord.name(), Some("in_texcoord0"));

    let tex_coord = entry.outputs().find(|var| var.location() == 1).unwrap();
    assert_eq!(tex_coord.storage_class(), spv::StorageClass::Output);
    assert_eq!(tex_coord.name(), Some("out_texcoord0"));

    let instances = module.uniforms()
        .find(|unif| (unif.set(), unif.binding()) == (0, 1))
        .unwrap();
    assert_eq!(instances.storage_class(), spv::StorageClass::Uniform);
}
