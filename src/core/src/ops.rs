/// Implements binary operators in terms of the by-value and
/// by-reference assignment operators.
macro_rules! impl_bin_ops {
    (
        $T:ident<$($LT:lifetime),*$(,)* $($A:ident),*$(,)*>,
        $Op:ident, $OpAssign:ident, $op:ident, $op_assign:ident$(,)*
    ) => {
        // LHS by val, RHS by val
        impl<$($LT,)* $($A,)*>
            ::std::ops::$Op<$T<$($LT,)* $($A,)*>>
            for $T<$($LT,)* $($A,)*>
        where $T<$($LT,)* $($A,)*>:
            ::std::ops::$OpAssign<$T<$($LT,)* $($A,)*>>,
        {
            type Output = $T<$($LT,)* $($A,)*>;
            fn $op(mut self, rhs: $T<$($LT,)* $($A,)*>) ->
                Self::Output
            {
                ::std::ops::$OpAssign::$op_assign(&mut self, rhs);
                self
            }
        }

        // LHS by val, RHS by ref
        impl<'rhs, $($LT,)* $($A,)*>
            ::std::ops::$Op<&'rhs $T<$($LT,)* $($A,)*>>
            for $T<$($LT,)* $($A,)*>
        where $T<$($LT,)* $($A,)*>:
            ::std::ops::$OpAssign<&'rhs $T<$($LT,)* $($A,)*>>
        {
            type Output = $T<$($LT,)* $($A,)*>;
            fn $op(mut self, rhs: &'rhs $T<$($LT,)* $($A,)*>) ->
                Self::Output
            {
                ::std::ops::$OpAssign::$op_assign(&mut self, rhs);
                self
            }
        }

        // LHS by ref, RHS by val
        impl<'lhs, $($LT,)* $($A,)*>
            ::std::ops::$Op<$T<$($LT,)* $($A,)*>>
            for &'lhs $T<$($LT,)* $($A,)*>
        where $T<$($LT,)* $($A,)*>:
            ::std::ops::$OpAssign<$T<$($LT,)* $($A,)*>>
            + Clone
        {
            type Output = $T<$($LT,)* $($A,)*>;
            fn $op(self, rhs: $T<$($LT,)* $($A,)*>) ->
                Self::Output
            {
                // TODO: This copy could be avoided by assigning to rhs
                // and returning it, but this only works when the
                // operator is commutative.
                let mut res = self.clone();
                ::std::ops::$OpAssign::$op_assign(&mut res, rhs);
                res
            }
        }

        // LHS by ref, RHS by ref
        impl<'lhs, 'rhs, $($LT,)* $($A,)*>
            ::std::ops::$Op<&'rhs $T<$($LT,)* $($A,)*>>
            for &'lhs $T<$($LT,)* $($A,)*>
        where $T<$($LT,)* $($A,)*>:
            ::std::ops::$OpAssign<&'rhs $T<$($LT,)* $($A,)*>> + Clone
            + Clone
        {
            type Output = $T<$($LT,)* $($A,)*>;
            fn $op(self, rhs: &'rhs $T<$($LT,)* $($A,)*>) ->
                Self::Output
            {
                let mut res = self.clone();
                ::std::ops::$OpAssign::$op_assign(&mut res, rhs);
                res
            }
        }
    };
}
