use std::path::{Path, PathBuf};

pub fn shader_dir() -> PathBuf {
    Path::new(file!())
        .parent()
        .unwrap()
        .join("../generated/shaders")
}
