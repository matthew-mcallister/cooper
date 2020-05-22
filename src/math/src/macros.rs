// derive_more doesn't work with const generics yet...
#[macro_export]
macro_rules! derive_index {
    (
        ($($params:tt)*), $ty:ty, $inner:ident, $inner_ty:ty,
    ) => {
        impl<Idx, $($params)*> std::ops::Index<Idx> for $ty
            where $inner_ty: std::ops::Index<Idx>
        {
            type Output = <$inner_ty as std::ops::Index<Idx>>::Output;
            #[inline(always)]
            fn index(&self, idx: Idx) -> &Self::Output {
                &self.$inner[idx]
            }
        }

        impl<Idx, $($params)*> std::ops::IndexMut<Idx> for $ty
            where $inner_ty: std::ops::IndexMut<Idx>
        {
            #[inline(always)]
            fn index_mut(&mut self, idx: Idx) -> &mut Self::Output {
                &mut self.$inner[idx]
            }
        }
    }
}
