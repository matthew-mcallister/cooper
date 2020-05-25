use base::impl_bin_ops;

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

#[macro_export]
macro_rules! impl_un_op {
    (
        {$($params:tt)*}, ($Ty:ty),
        $Op:ident, $op:ident
    ) => {
        impl<$($params)*> $Op for $Ty {
            type Output = $Ty;
            #[inline(always)]
            fn $op(mut self) -> Self::Output {
                for x in self.iter_mut() {
                    *x = $Op::$op(&*x);
                }
                self
            }
        }

        impl<'a, $($params)*> $Op for &'a $Ty {
            type Output = $Ty;
            #[inline(always)]
            fn $op(self) -> Self::Output {
                let mut res = Self::Output::default();
                for (dst, &src) in res.iter_mut().zip(self.iter()) {
                    *dst = $Op::$op(src);
                }
                res
            }
        }
    }
}

#[macro_export]
macro_rules! impl_bin_op {
    (
        {$($params:tt)*}, ($Ty:ty),
        $Op:ident, $OpAssign:ident, $op:ident, $op_assign:ident
    ) => {
        impl<$($params)*> $OpAssign for $Ty {
            #[inline(always)]
            fn $op_assign(&mut self, other: Self) {
                for (dst, src) in self.iter_mut().zip(other.iter()) {
                    dst.$op_assign(src);
                }
            }
        }

        impl<'rhs, $($params)*> $OpAssign<&'rhs Self> for $Ty {
            #[inline(always)]
            fn $op_assign(&mut self, other: &'rhs Self) {
                for (dst, src) in self.iter_mut().zip(other.iter()) {
                    dst.$op_assign(src);
                }
            }
        }

        impl_bin_ops!(
            {$($params)*},
            ($Ty), ($Ty),
            copy,
            (std::ops::$Op), (std::ops::$OpAssign), $op, $op_assign,
        );
    }
}

macro_rules! impl_scalar_op {
    (
        {$($params:tt)*}, ($Lhs:ty), ($Rhs:ty),
        $Op:ident, $OpAssign:ident, $op:ident, $op_assign:ident
    ) => {
        impl<$($params)*> $OpAssign<$Rhs> for $Lhs {
            #[inline(always)]
            fn $op_assign(&mut self, rhs: $Rhs) {
                for dst in self.iter_mut() {
                    dst.$op_assign(rhs);
                }
            }
        }

        impl<'rhs, $($params)*> $OpAssign<&'rhs $Rhs> for $Lhs {
            #[inline(always)]
            fn $op_assign(&mut self, rhs: &'rhs $Rhs) {
                for dst in self.iter_mut() {
                    dst.$op_assign(rhs);
                }
            }
        }

        impl_bin_ops!(
            {$($params)*}, ($Lhs), ($Rhs), copy,
            (std::ops::$Op), (std::ops::$OpAssign), $op, $op_assign,
        );
    }
}
