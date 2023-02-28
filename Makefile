.RECIPEPREFIX := >

CARGO := cargo
SHADERC := glslc
SHADERC_FLAGS ?=

SHADER_SRC_DIR := shaders
SHADER_OBJ_DIR := generated/shaders

SHADER_SRC_FILES := \
    triangle_vert.glsl \
    triangle_frag.glsl
SHADER_SRCS := $(patsubst %,$(SHADER_SRC_DIR)/%,$(SHADER_SRC_FILES))

SHADER_HDR_FILES :=
SHADER_HDRS := $(patsubst %,$(SHADER_SRC_DIR)/%,$(SHADER_HDR_FILES))

SHADER_OBJS := $(patsubst %.glsl,$(SHADER_OBJ_DIR)/%.spv,$(SHADER_SRC_FILES))

.PHONY: shaders
shaders: $(SHADER_OBJS)

$(SHADER_OBJ_DIR)/%.spv: \
    $(SHADER_SRC_DIR)/%.glsl \
    $(SHADER_HDRS) \
    $(SHADER_OBJ_DIR)
> $(SHADERC) $(SHADERC_FLAGS) $< -o $@

$(SHADER_OBJ_DIR):
> mkdir -p $(SHADER_OBJ_DIR)

.PHONY: clean
clean:
> rm -r $(SHADER_OBJ_DIR)
