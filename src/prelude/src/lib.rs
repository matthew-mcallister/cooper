pub use self::opt as guard;

#[macro_export]
macro_rules! c_str {
    ($($str:expr),*$(,)*) => {
        concat!($($str,)* "\0") as *const str as *const std::os::raw::c_char
    };
}

#[macro_export]
macro_rules! insert_unique {
    ($map:expr, $key:expr, $val:expr) => {
        assert!($map.insert($key, $val).is_none());
    }
}

pub fn opt(b: bool) -> Option<()> {
    if b { Some(()) } else { None }
}

pub type AnyError = Box<dyn std::error::Error>;

pub trait ResultExt<T, E> {
    /// Executes a callback if the result was an error and returns the
    /// result unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// # use cooper_prelude::ResultExt;
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

/// Returns the smallest multiple of `alignment` that is `>= offset`.
#[inline(always)]
pub fn align<T: Copy + num::PrimInt>(alignment: T, offset: T) -> T {
    ((offset + alignment - T::one()) / alignment) * alignment
}

pub trait SliceExt {
    type Target: Sized;

    /// Casts a slice to a byte array.
    fn as_bytes(&self) -> &[u8];

    /// Casts a slice to a mutable byte array.
    fn as_bytes_mut(&mut self) -> &mut [u8];

    /// Converts a slice to a *non-dangling* pointer. This means that,
    /// if the slice has length zero, the returned pointer is NULL.
    /// Though it is hardly undocumented, this is not the case for
    /// `slice::as_ptr`.
    fn c_ptr(&self) -> *const Self::Target;
}

impl<T> SliceExt for [T] {
    type Target = T;

    #[inline(always)]
    fn as_bytes(&self) -> &[u8] {
        let len = self.len() * std::mem::size_of::<T>();
        unsafe { std::slice::from_raw_parts(self as *const [T] as _, len) }
    }

    #[inline(always)]
    fn as_bytes_mut(&mut self) -> &mut [u8] {
        let len = self.len() * std::mem::size_of::<T>();
        unsafe { std::slice::from_raw_parts_mut(self as *mut [T] as _, len) }
    }

    #[inline(always)]
    fn c_ptr(&self) -> *const Self::Target {
        if self.is_empty() { std::ptr::null() } else { self.as_ptr() }
    }
}

pub trait AsPtr {
    type Target;

    fn as_ptr(self) -> *const Self::Target;
}

impl<'a, T> AsPtr for Option<&'a T> {
    type Target = T;

    #[inline(always)]
    fn as_ptr(self) -> *const Self::Target {
        unsafe { std::mem::transmute(self) }
    }
}
