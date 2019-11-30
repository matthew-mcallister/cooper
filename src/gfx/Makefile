include ../../common.mk

SHADER_SRC_DIR := shaders
SHADER_OBJ_DIR := generated/shaders

SHADER_SRC_FILES := \
    example_vert.glsl \
    example_frag.glsl \
    triangle_vert.glsl \
    triangle_frag.glsl
SHADER_SRCS := $(patsubst %,$(SHADER_SRC_DIR)/%,$(SHADER_SRC_FILES))
SHADER_HEADER_FILES := \
    example_inc.glsl
SHADER_HEADERS := $(patsubst %,$(SHADER_SRC_DIR)/%,$(SHADER_HEADER_FILES))
  
SHADER_OBJS := $(patsubst %.glsl,$(SHADER_OBJ_DIR)/%.spv,$(SHADER_SRC_FILES))

.PHONY: dev
dev: $(SHADER_OBJS)
> VK_INSTANCE_LAYERS=VK_LAYER_LUNARG_standard_validation $(CARGO) run

.PHONY: shaders
shaders: $(SHADER_OBJS)

$(SHADER_OBJ_DIR)/%.spv: \
    $(SHADER_SRC_DIR)/%.glsl \
    $(SHADER_HEADERS) \
    $(SHADER_OBJ_DIR)
> glslc -O $< -o $@

$(SHADER_OBJ_DIR):
> mkdir -p $(SHADER_OBJ_DIR)
