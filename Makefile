SHADER_SRC_DIR = data/shaders
SHADER_MOD_DIR = data/spirv

SHADER_SRCS = $(wildcard $(SHADER_SRC_DIR)/*.glsl)
SHADER_MODS = $(patsubst $(SHADER_SRC_DIR)/%.glsl,$(SHADER_MOD_DIR)/%.spv,$(SHADER_SRCS))

.PHONY: shaders
shaders: $(SHADER_MODS)

$(SHADER_MOD_DIR)/%.spv: $(SHADER_SRC_DIR)/%.glsl $(SHADER_MOD_DIR)
	glslc $< -o $@

$(SHADER_MOD_DIR):
	mkdir -p $(SHADER_MOD_DIR)
