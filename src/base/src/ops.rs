/// Implements binary operators in terms of the by-value and
/// by-reference assignment operators.
///
/// Don't use Self as a parameter.
#[macro_export]
macro_rules! impl_bin_ops {
    (
        $({$($parms:tt)+},)? ($($lhs:tt)+), ($($rhs:tt)+), $clone:ident,
        ($($Op:tt)+), ($($OpAssign:tt)+), $op:ident, $op_assign:ident$(,)*
    ) => {
        // LHS by val, RHS by val
        impl$(<$($parms)*>)? $($Op)*<$($rhs)*> for $($lhs)*
            where $($lhs)*: $($OpAssign)*<$($rhs)*>,
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
            where $($lhs)*: $($OpAssign)*<&'rhs $($rhs)*>
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
            where $($lhs)*: $($OpAssign)*<$($rhs)*> + Clone
        {
            type Output = $($lhs)*;
            #[inline(always)]
            fn $op(self, rhs: $($rhs)*) -> Self::Output {
                // TODO: This clone/copy isn't necessary if the operator
                // is commutative.
                let mut res = impl_bin_ops!(@$clone self);
                $($OpAssign)*::<$($rhs)*>::$op_assign(&mut res, rhs);
                res
            }
        }

        // LHS by ref, RHS by ref
        impl<'lhs, 'rhs, $($($parms)*)?>
            $($Op)*<&'rhs $($rhs)*> for &'lhs $($lhs)*
            where $($lhs)*: $($OpAssign)*<&'rhs $($rhs)*> + Clone
        {
            type Output = $($lhs)*;
            #[inline(always)]
            fn $op(self, rhs: &'rhs $($rhs)*) -> Self::Output {
                let mut res = impl_bin_ops!(@$clone self);
                $($OpAssign)*::<&'rhs $($rhs)*>::$op_assign
                    (&mut res, rhs);
                res
            }
        }
    };
    (@clone $expr:expr) => {
        $expr.clone()
    };
    (@copy $expr:expr) => {
        *$expr
    };
}
