/// Implements binary operators in terms of the by-value and
/// by-reference assignment operators.
///
/// Don't use Self as a parameter.
#[macro_export]
macro_rules! impl_bin_ops {
    (
        $({$($parms:tt)+}, {$($where:tt)*},)?
        ($($lhs:tt)+), ($($rhs:tt)+),
        $Clone:ident,
        ($($Op:tt)+), ($($OpAssign:tt)+), $op:ident, $op_assign:ident$(,)*
    ) => {
        // LHS by val, RHS by val
        impl$(<$($parms)*>)? $($Op)*<$($rhs)*> for $($lhs)*
        where
            $($lhs)*: $($OpAssign)*<$($rhs)*>,
            $($($where)*)?
        {
            type Output = $($lhs)*;
            #[inline(always)]
            fn $op(mut self, rhs: $($rhs)*) -> Self::Output {
                $($OpAssign)*::<$($rhs)*>::$op_assign(&mut self, rhs);
                self
            }
        }

        // LHS by val, RHS by ref
        impl<'rhs, $($($parms)*)?> $($Op)*<&'rhs $($rhs)*> for $($lhs)*
        where
            $($lhs)*: $($OpAssign)*<&'rhs $($rhs)*>,
            $($($where)*)?
        {
            type Output = $($lhs)*;
            #[inline(always)]
            fn $op(mut self, rhs: &'rhs $($rhs)*) -> Self::Output {
                $($OpAssign)*::<&'rhs $($rhs)*>::$op_assign
                    (&mut self, rhs);
                self
            }
        }

        // LHS by ref, RHS by val
        impl<'lhs, $($($parms)*)?> $($Op)*<$($rhs)*> for &'lhs $($lhs)*
        where
            $($lhs)*: $($OpAssign)*<$($rhs)*> + $Clone,
            $($($where)*)?
        {
            type Output = $($lhs)*;
            #[inline(always)]
            fn $op(self, rhs: $($rhs)*) -> Self::Output {
                // TODO: This clone/copy isn't necessary if the operator
                // is commutative.
                let mut res = $crate::impl_bin_ops!(@$Clone self);
                $($OpAssign)*::<$($rhs)*>::$op_assign(&mut res, rhs);
                res
            }
        }

        // LHS by ref, RHS by ref
        impl<'lhs, 'rhs, $($($parms)*)?>
            $($Op)*<&'rhs $($rhs)*> for &'lhs $($lhs)*
        where
            $($lhs)*: $($OpAssign)*<&'rhs $($rhs)*> + $Clone,
            $($($where)*)?
        {
            type Output = $($lhs)*;
            #[inline(always)]
            fn $op(self, rhs: &'rhs $($rhs)*) -> Self::Output {
                let mut res = $crate::impl_bin_ops!(@$Clone self);
                $($OpAssign)*::<&'rhs $($rhs)*>::$op_assign
                    (&mut res, rhs);
                res
            }
        }
    };
    (@Clone $expr:expr) => {
        $expr.clone()
    };
    (@Copy $expr:expr) => {
        *$expr
    };
}
