include ../../common.mk

SHADER_SRC_DIR := shaders
SHADER_OBJ_DIR := generated/shaders

SHADER_SRCS := $(wildcard $(SHADER_SRC_DIR)/*.glsl)
SHADER_OBJS := $(patsubst $(SHADER_SRC_DIR)/%.glsl,$(SHADER_OBJ_DIR)/%.spv,$(SHADER_SRCS))

.PHONY: shaders
shaders: $(SHADER_OBJS)

$(SHADER_OBJ_DIR)/%.spv: $(SHADER_SRC_DIR)/%.glsl $(SHADER_OBJ_DIR)
> glslc -O $< -o $@

$(SHADER_OBJ_DIR):
> mkdir -p $(SHADER_OBJ_DIR)
