use num_traits as num;

#[macro_export]
macro_rules! c_str {
    ($($str:expr),*) => {
        c_str!($($str,)*)
    };
    ($($str:expr,)*) => {
        concat!($($str,)* "\0") as *const str as *const std::os::raw::c_char
    };
}

pub type AnyError = Box<dyn std::error::Error>;

pub trait ResultExt<T, E> {
    /// Executes a callback if the result was an error and returns the
    /// result unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// # use cooper_core::prelude::ResultExt;
    /// # fn sthg() -> Result<(), u32> {
    /// # let result = Err(0u32);
    /// result.on_err(|x| println!("error: {}", x))?;
    /// # Ok(())
    /// # }
    /// ```
    fn on_err(self, f: impl FnOnce(&E)) -> Self;
}

impl<T, E> ResultExt<T, E> for Result<T, E> {
    #[inline(always)]
    fn on_err(self, f: impl FnOnce(&E)) -> Self {
        self.as_ref().err().map(f);
        self
    }
}

// Returns the smallest multiple of `alignment` that is `>= offset`.
#[inline(always)]
pub fn align<T: Copy + num::Num>(alignment: T, offset: T) -> T {
    ((offset + alignment - num::one()) / alignment) * alignment
}

// A.k.a. guard
#[inline(always)]
pub fn opt(cond: bool) -> Option<()> {
    if cond { Some(()) } else { None }
}

// Vexing that this isn't in std
#[inline(always)]
pub fn slice_to_bytes<T: Sized>(slice: &[T]) -> &[u8] {
    let len = slice.len() * std::mem::size_of::<T>();
    unsafe { std::slice::from_raw_parts(slice as *const [T] as _, len) }
}
