EXAMPLE_GLSL := examples/example.glsl
EXAMPLE_SPV := build/example.spv
GENERATED_RS := generated/generated.rs

GENERATOR := bin/generate
GLSL := glslc
GLSL_FLAGS := -g

SPIRV_JSON := vendor/SPIRV-Headers/include/spirv/unified1/spirv.json

.PHONY: all
all: gen shaders

.PHONY: gen
gen: $(GENERATED_RS)

$(GENERATED_RS): $(SPIRV_JSON) $(GENERATOR)
	$(GENERATOR) < $< > $@

.PHONY: shaders
shaders: $(EXAMPLE_SPV)

$(EXAMPLE_SPV): $(EXAMPLE_GLSL)
	$(GLSL) $(GLSL_FLAGS) -o $@ $(EXAMPLE_GLSL)
