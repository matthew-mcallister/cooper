use itertools::{Itertools, MinMaxResult};

/// See `MathIterExt::infsup`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InfSupResult<T> {
    Empty,
    Singleton(T),
    InfSup(T, T),
}

impl<T> From<InfSupResult<T>> for Result<(T, T), Option<T>> {
    fn from(res: InfSupResult<T>) -> Self {
        match res {
            InfSupResult::Empty => Err(None),
            InfSupResult::Singleton(x) => Err(Some(x)),
            InfSupResult::InfSup(x, y) => Ok((x, y)),
        }
    }
}

impl<T> InfSupResult<T> {
    fn from_minmax(minmax: MinMaxResult<T>) -> Self {
        match minmax {
            MinMaxResult::NoElements => Self::Empty,
            MinMaxResult::OneElement(x) => Self::Singleton(x),
            MinMaxResult::MinMax(x, y) => Self::InfSup(x, y),
        }
    }

    /// Converts the `InfSupResult` into a `Result`. `InfSup(x, y)` maps
    /// to `Ok((x, y))`, `Singleton(x)` maps to `Err(Some(x))`, and
    /// `Empty` maps to `Err(None)`.
    pub fn into_result(self) -> Result<(T, T), Option<T>> {
        self.into()
    }
}

/// This trait allows taking the infimum and supremum over the items of
/// an iterator. It is used by the `inf`, `sup`, and `infsup` methods of
/// `MathIterExt`.
pub trait InfSup<A = Self>: Sized {
    /// Returns the infimum of the items of an iterator.
    fn infimum(iter: impl Iterator<Item = A>) -> Option<Self>;

    /// Returns the supremum of the items of an iterator.
    fn supremum(iter: impl Iterator<Item = A>) -> Option<Self>;

    /// Returns the infimum and supremum of the items of an iterator.
    fn infimum_and_supremum(iter: impl Iterator<Item = A>) ->
        InfSupResult<Self>;
}

impl<T: Ord + Clone> InfSup for T {
    fn infimum(iter: impl Iterator<Item = Self>) -> Option<Self> {
        iter.min()
    }

    fn supremum(iter: impl Iterator<Item = Self>) -> Option<Self> {
        iter.max()
    }

    fn infimum_and_supremum(iter: impl Iterator<Item = Self>) ->
        InfSupResult<Self>
    {
        InfSupResult::from_minmax(iter.minmax())
    }
}

/// Math extensions for the `Iterator` trait.
pub trait MathIterExt: Iterator + Sized {
    /// Returns the infimum of the items of an iterator if it is
    /// nonempty, else `None`.
    fn inf<T>(self) -> Option<T>
        where T: InfSup<Self::Item>,
    {
        T::infimum(self)
    }

    /// Returns the supremum of the items of an iterator if it is
    /// nonempty, else `None`.
    fn sup<T>(self) -> Option<T>
        where T: InfSup<Self::Item>,
    {
        T::supremum(self)
    }

    /// Returns the infimum and supremum of the items of an iterator.
    /// If the iterator is empty, returns `Empty`, and, if the iterator
    /// produces only one item, returns `Singleton`.
    fn infsup<T>(self) -> InfSupResult<T>
        where T: InfSup<Self::Item>
    {
        T::infimum_and_supremum(self)
    }
}

impl<I: Iterator> MathIterExt for I {}
