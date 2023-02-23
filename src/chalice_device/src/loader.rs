//! Loads Vulkan from the system lib.

use libloading::{library_filename, Error, Library, Symbol};

pub fn load_vulkan() -> Result<vk::pfn::GetInstanceProcAddr, Error> {
    unsafe {
        let lib = Library::new(library_filename("vulkan"))?;
        let sym: Symbol<vk::pfn::GetInstanceProcAddr> = lib.get(b"vkGetInstanceProcAddr")?;
        let ptr = sym.into_raw().into_raw();
        // Keep the library loaded
        std::mem::forget(lib);
        Ok(std::mem::transmute(ptr))
    }
}
