/// Implements binary operators in terms of the by-value and
/// by-reference assignment operators.
///
/// Don't use Self as a parameter.
// Don't use Self in the macro definition either.
#[macro_export]
macro_rules! impl_bin_ops {
    (
        $({$($parms:tt)+},)? ($($lhs:tt)+), ($($rhs:tt)+),
        $Op:ident, $OpAssign:ident, $op:ident, $op_assign:ident$(,)*
    ) => {
        // LHS by val, RHS by val
        impl$(<$($parms)*>)? ::std::ops::$Op<$($rhs)*> for $($lhs)*
            where $($lhs)*: ::std::ops::$OpAssign<$($rhs)*>,
        {
            type Output = $($lhs)*;
            fn $op(mut self, rhs: $($rhs)*) -> Self::Output {
                ::std::ops::$OpAssign::<$($rhs)*>::$op_assign(&mut self, rhs);
                self
            }
        }

        // LHS by val, RHS by ref
        impl<'rhs, $($($parms)*)?> ::std::ops::$Op<&'rhs $($rhs)*> for $($lhs)*
            where $($lhs)*: ::std::ops::$OpAssign<&'rhs $($rhs)*>
        {
            type Output = $($lhs)*;
            fn $op(mut self, rhs: &'rhs $($rhs)*) -> Self::Output {
                ::std::ops::$OpAssign::<&'rhs $($rhs)*>::$op_assign
                    (&mut self, rhs);
                self
            }
        }

        // LHS by ref, RHS by val
        impl<'lhs, $($($parms)*)?> ::std::ops::$Op<$($rhs)*> for &'lhs $($lhs)*
            where $($lhs)*: ::std::ops::$OpAssign<$($rhs)*> + Clone
        {
            type Output = $($lhs)*;
            fn $op(self, rhs: $($rhs)*) -> Self::Output {
                // TODO: This clone could be avoided by assigning to rhs
                // and returning it, but this only works when the
                // operator is commutative.
                let mut res = self.clone();
                ::std::ops::$OpAssign::<$($rhs)*>::$op_assign(&mut res, rhs);
                res
            }
        }

        // LHS by ref, RHS by ref
        impl<'lhs, 'rhs, $($($parms)*)?>
            ::std::ops::$Op<&'rhs $($rhs)*> for &'lhs $($lhs)*
            where $($lhs)*: ::std::ops::$OpAssign<&'rhs $($rhs)*> + Clone
        {
            type Output = $($lhs)*;
            fn $op(self, rhs: &'rhs $($rhs)*) -> Self::Output {
                let mut res = self.clone();
                ::std::ops::$OpAssign::<&'rhs $($rhs)*>::$op_assign
                    (&mut res, rhs);
                res
            }
        }
    };
}
