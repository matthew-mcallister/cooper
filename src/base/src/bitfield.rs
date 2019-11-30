use std::ops::Range;

// N.B.: out-of-bounds indexing will cause a panic on shift overflow.
pub trait BitField {
    fn get_bits(self, range: Range<u8>) -> Self;
    fn set_bits(self, range: Range<u8>, value: Self) -> Self;
    fn get_bit(self, index: u8) -> bool;
    fn set_bit(self, index: u8, value: bool) -> Self;
}

macro_rules! impl_num_ext {
    ($type:ty) => {
        impl BitField for $type {
            #[inline(always)]
            fn get_bits(self, range: Range<u8>) -> Self {
                let (lsb, msb) = (range.start, range.end);
                let bit_count = (std::mem::size_of::<$type>() * 8) as u8;
                let mask = !0 >> (bit_count - (msb - lsb));
                (self >> lsb) & mask
            }

            #[inline(always)]
            fn set_bits(self, range: Range<u8>, value: Self) -> Self {
                let (lsb, msb) = (range.start, range.end);
                let bit_count = (std::mem::size_of::<$type>() * 8) as u8;
                let mask = (!0 >> (bit_count - (msb - lsb))) << lsb;
                (self & !mask) | ((value << lsb) & mask)
            }

            #[inline(always)]
            fn get_bit(self, index: u8) -> bool {
                self & (1 << index) != 0
            }

            #[inline(always)]
            fn set_bit(self, index: u8, value: bool) -> Self {
                let value = value as Self;
                let mask = !(1 << index);
                (self & mask) | (value << index)
            }
        }
    }
}

impl_num_ext!(u8);
impl_num_ext!(u16);
impl_num_ext!(u32);
impl_num_ext!(u64);
impl_num_ext!(u128);

#[cfg(test)]
mod num_ext_tests {
    use super::*;

    // TODO: More tests, obviously
    #[test]
    fn smoke_tests() {
        // u8
        let sample = 0b11001001u8;
        assert!(sample.get_bit(7));
        assert!(!sample.get_bit(2));
        assert_eq!(sample.set_bit(0, true), sample);
        assert_eq!(sample.set_bit(7, false), 0b01001001);
        assert_eq!(sample.set_bit(2, true), 0b11001101);

        assert_eq!(sample.get_bits(2..4), 0b10);
        assert_eq!(sample.get_bits(0..8), 0b11001001);
        assert_eq!(sample.set_bits(2..4, 0b11), 0b11001101);
        assert_eq!(sample.set_bits(0..8, 0), 0);

        // u32
        let sample = 0xFF00DD00u32;
        assert!(sample.get_bit(8));
        assert!(!sample.get_bit(16));
        assert_eq!(sample.set_bit(0, true), 0xFF00DD01);
        assert_eq!(sample.set_bit(0, false), sample);
        assert_eq!(sample.set_bit(8, false), 0xFF00DC00);

        assert_eq!(sample.get_bits(8..16), 0xDD);
        assert_eq!(sample.set_bits(16..24, 0x12), 0xFF12DD00);
        assert_eq!(sample.set_bits(24..32, 0x12), 0x1200DD00);
    }
}

/// A basic implementation of bit fields.
// TODO: read-only fields
#[macro_export]
macro_rules! bitfield {
    (
        $(#[$($meta:tt)*])*
        $st_vis:vis struct $st_name:ident($bit_ty:ty) {
            $({ $($field_inner:tt)* },)*
        }
        $($rest:tt)*
    ) => {
        $(#[$($meta)*])*
        $st_vis struct $st_name {
            bits: $bit_ty,
        }

        impl $st_name {
            $(bitfield!(@field {$($field_inner)*});)*
        }

        bitfield! {
            @option [[
                $st_vis struct $st_name($bit_ty) {
                    $({ $($field_inner)* },)*
                }
            ]]
            $($rest)*
        }
    };
    (
        @option [[
            $st_vis:vis struct $st_name:ident($bit_ty:ty) {
                $({
                    getter: $get_vis:vis $getter:ident,
                    $($unused:tt)*
                },)*
            }
        ]]
        impl Debug;
        $($rest:tt)*
    ) => {
        impl std::fmt::Debug for $st_name {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.debug_struct(stringify!($st_name))
                    $(.field(stringify!($getter), &self.$getter()))*
                    .finish()
            }
        }

        bitfield! {
            @option [[
                $st_vis struct $st_name($bit_ty) {
                    $({ $($field_inner)* },)*
                }
            ]]
            $($rest)*
        }
    };
    (
        @field {
            getter: $get_vis:vis $getter:ident,
            setter: $set_vis:vis $setter:ident,
            type: $type:ty,
            bits: ($lsb:tt, $msb:tt),
        }
    ) => {
        $get_vis fn $getter(&self) -> $type {
            $crate::bitfield::BitField::get_bits(self.bits, $lsb..$msb) as _
        }

        $set_vis fn $setter(&mut self, value: $type) {
            self.bits = $crate::bitfield::BitField::set_bits
                (self.bits, $lsb..$msb, value as _)
        }
    };
    (
        @field {
            getter: $get_vis:vis $getter:ident,
            setter: $set_vis:vis $setter:ident,
            type: bool,
            bit: $bit:tt,
        }
    ) => {
        $get_vis fn $getter(&self) -> bool {
            $crate::bitfield::BitField::get_bit(self.bits, $bit)
        }

        $set_vis fn $setter(&mut self, value: bool) {
            self.bits = $crate::bitfield::BitField::set_bit
                (self.bits, $bit, value)
        }
    };
    (
        @option [[
            $st_vis:vis struct $st_name:ident($bit_ty:ty) {
                $({
                    getter: $get_vis:vis $getter:ident,
                    $($field_rest:tt)*
                },)*
            }
        ]]
        impl Debug;
        $($rest:tt)*
    ) => {
        bitfield! {
            @option [[
                $st_vis struct $st_name($bit_ty) {
                    $({
                        getter: $get_vis $getter,
                        $($field_rest)*
                    },)*
                }
            ]]
            $($rest)*
        }
    };
    (@option [[$($stuff:tt)*]]) => {};
}

#[cfg(test)]
mod test_bitfield {
    bitfield! {
        #[derive(Clone, Copy)]
        pub struct TestField(u32) {
            {
                getter: pub lower,
                setter: pub set_lower,
                type: u16,
                bits: (0, 16),
            },
            {
                getter: pub upper,
                setter: pub set_upper,
                type: u16,
                bits: (16, 32),
            },
            {
                getter: pub top,
                setter: pub set_top,
                type: bool,
                bit: 31,
            },
        }
    }

    #[test]
    fn smoke_tests() {
        let mut bf = TestField { bits: 0 };

        assert_eq!(bf.lower(), 0);
        assert_eq!(bf.upper(), 0);
        assert!(!bf.top());

        bf.set_lower(5);
        assert_eq!(bf.bits, 5);

        bf.set_upper(5);
        assert_eq!(bf.bits, 0x00050005);

        bf.set_top(true);
        assert_eq!(bf.bits, 0x80050005);

        assert_eq!(bf.lower(), 5);
        assert_eq!(bf.upper(), 0x8005);
        assert!(bf.top());
    }
}
