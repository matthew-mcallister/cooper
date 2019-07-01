struct SpriteTransform {
    mat2 mat;
    vec2 offs;
};

struct Sprite {
    SpriteTransform transform;
    uvec2 textures;
};
