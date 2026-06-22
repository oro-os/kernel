//! Module for trait implementations of saturating casts.
use core::{i128, i16, i32, i64, i8, isize};
use core::{u128, u16, u32, u64, u8, usize};

/// Trait that enables saturating casts between a source and target type. The
/// value is not preserved if the target type cannot represent the original
/// source value. This cast does not fail.
///
/// A saturating cast is similar to performing the `clamp` function on a value
/// before casting. If the source value is less than the target minimum, then
/// the target minimum is returned. If the source value is greater than the
/// target maximum, then the target maximum is returned.
///
/// See [`SaturatingElement`] for how to implement saturating traits for custom
/// types.
///
/// ```
/// use saturating_cast::SaturatingCast;
///
/// let x: i64 = 1024;
/// let y: u8 = 10;
/// let z = x.saturating_cast::<u8>() - y;
/// assert_eq!(245, z);
///
/// let a = -128_i8;
/// let b: u16 = a.saturating_cast();
/// assert_eq!(0, b);
/// ```
pub trait SaturatingCast {
    /// Performs a saturating cast to the target type `T`.
    #[inline]
    fn saturating_cast<T>(self) -> T
    where
        Self: SaturatingElement<T>,
    {
        SaturatingElement::as_element(self)
    }
}

impl SaturatingCast for u8 {}
impl SaturatingCast for u16 {}
impl SaturatingCast for u32 {}
impl SaturatingCast for u64 {}
impl SaturatingCast for u128 {}
impl SaturatingCast for usize {}

impl SaturatingCast for i8 {}
impl SaturatingCast for i16 {}
impl SaturatingCast for i32 {}
impl SaturatingCast for i64 {}
impl SaturatingCast for i128 {}
impl SaturatingCast for isize {}

/// Supporting trait for [`SaturatingCast`] which performs saturating conversion
/// from a source element type to a target element type.
///
/// The following code shows how to implement traits that allow for saturating
/// casts from the custom type `Int` to another custom type, `Uint`.
///
/// ```
/// use saturating_cast::{SaturatingCast, SaturatingElement};
///
/// #[derive(Clone, Copy)]
/// struct Int(i32);
///
/// struct Uint(u8);
///
/// impl SaturatingElement<Uint> for Int {
///     fn as_element(self) -> Uint {
///         Uint(self.0.max(0).min(255) as u8)
///     }
/// }
///
/// impl SaturatingCast for Int {}
///
/// assert_eq!(u8::MIN, Int(i32::MIN).saturating_cast::<Uint>().0);
/// assert_eq!(u8::MAX, Int(512).saturating_cast::<Uint>().0);
pub trait SaturatingElement<T>: Copy {
    /// Clamp `self` to within the range of `T::MIN..=T::MAX`, then return that
    /// value cast to the target type `T`.
    fn as_element(self) -> T;
}

// Unsigned integer saturating conversions
macro_rules! impl_lossless_casts {
    ($src: ty => $($target: ty),*) => {$(
        impl SaturatingElement<$target> for $src {
            #[inline]
            fn as_element(self) -> $target {
                <$target>::from(self)
            }
        }
    )*};
}

macro_rules! impl_uint_clamp_to_max_bound {
    ($src: ty => $($target: ty),*) => {$(
        impl SaturatingElement<$target> for $src {
            #[inline]
            fn as_element(self) -> $target {
                self.min(<$target>::MAX as $src) as $target
            }
        }
    )*};
}

// u8
impl_lossless_casts!(u8 => u8, u16, u32, u64, u128, usize);
impl_lossless_casts!(u8 => i16, i32, i64, i128, isize);
impl_uint_clamp_to_max_bound!(u8 => i8);

// u16
impl_lossless_casts!(u16 => u16, u32, u64, u128, usize);
impl_lossless_casts!(u16 => i32, i64, i128);
impl_uint_clamp_to_max_bound!(u16 => u8);
impl_uint_clamp_to_max_bound!(u16 => i8, i16, isize);

// u32
impl_lossless_casts!(u32 => u32, u64, u128);
impl_lossless_casts!(u32 => i64, i128);
impl_uint_clamp_to_max_bound!(u32 => u8, u16, usize);
impl_uint_clamp_to_max_bound!(u32 => i8, i16, i32, isize);

// u64
impl_lossless_casts!(u64 => u64, u128);
impl_lossless_casts!(u64 => i128);
impl_uint_clamp_to_max_bound!(u64 => u8, u16, u32, usize);
impl_uint_clamp_to_max_bound!(u64 => i8, i16, i32, i64, isize);

// u128
impl_lossless_casts!(u128 => u128);
impl_uint_clamp_to_max_bound!(u128 => u8, u16, u32, u64, usize);
impl_uint_clamp_to_max_bound!(u128 => i8, i16, i32, i64, i128, isize);

// usize
impl_lossless_casts!(usize => usize);
impl_uint_clamp_to_max_bound!(usize => u8, u16, u32, u64, u128);
impl_uint_clamp_to_max_bound!(usize => i8, i16, i32, i64, i128, isize);

// Signed integer saturating conversions

// Conversions where the bit-width of int max and uint max fit within the source
macro_rules! impl_int_clamp_to_smaller {
    ($src: ty => $($target: ty),*) => {$(
        impl SaturatingElement<$target> for $src {
            #[inline]
            fn as_element(self) -> $target {
                self.max(<$target>::MIN as $src).min(<$target>::MAX as $src) as $target
            }
        }
    )*};
}

// Conversions like i8 to u8, where i8::MAX < u8::MAX
macro_rules! impl_int_clamp_to_zero_with_larger_uint_max {
    ($src: ty => $($target: ty),*) => {$(
        impl SaturatingElement<$target> for $src {
            #[inline]
            fn as_element(self) -> $target {
                self.max(0) as $target
            }
        }
    )*};
}

// Conversions to isize and usize for i32, i64, and i128.
macro_rules! impl_int_isize_usize {
    ($src: ty) => {
        #[cfg(target_pointer_width = "16")]
        impl SaturatingElement<isize> for $src {
            #[inline]
            fn as_element(self) -> isize {
                self.saturating_cast::<i16>() as isize
            }
        }

        #[cfg(target_pointer_width = "32")]
        impl SaturatingElement<isize> for $src {
            #[inline]
            fn as_element(self) -> isize {
                self.saturating_cast::<i32>() as isize
            }
        }

        #[cfg(target_pointer_width = "64")]
        impl SaturatingElement<isize> for $src {
            #[inline]
            fn as_element(self) -> isize {
                self.saturating_cast::<i64>() as isize
            }
        }

        #[cfg(target_pointer_width = "16")]
        impl SaturatingElement<usize> for $src {
            #[inline]
            fn as_element(self) -> usize {
                self.saturating_cast::<u16>() as usize
            }
        }

        #[cfg(target_pointer_width = "32")]
        impl SaturatingElement<usize> for $src {
            #[inline]
            fn as_element(self) -> usize {
                self.saturating_cast::<u32>() as usize
            }
        }

        #[cfg(target_pointer_width = "64")]
        impl SaturatingElement<usize> for $src {
            #[inline]
            fn as_element(self) -> usize {
                self.saturating_cast::<u64>() as usize
            }
        }
    };
}

macro_rules! impl_isize_casts {
    ($($target: ty),*) => {$(
        #[cfg(target_pointer_width = "16")]
        impl SaturatingElement<$target> for isize {
            #[inline]
            fn as_element(self) -> $target {
                (self as i16).saturating_cast::<$target>()
            }
        }

        #[cfg(target_pointer_width = "32")]
        impl SaturatingElement<$target> for isize {
            #[inline]
            fn as_element(self) -> $target {
                (self as i32).saturating_cast::<$target>()
            }
        }

        #[cfg(target_pointer_width = "64")]
        impl SaturatingElement<$target> for isize {
            #[inline]
            fn as_element(self) -> $target {
                (self as i64).saturating_cast::<$target>()
            }
        }
    )*};
}

// i8
impl_lossless_casts!(i8 => i8, i16, i32, i64, i128, isize);
impl_int_clamp_to_zero_with_larger_uint_max!(i8 => u8, u16, u32, u64, u128, usize);

// i16
impl_lossless_casts!(i16 => i16, i32, i64, i128, isize);
impl_int_clamp_to_smaller!(i16 => i8);
impl_int_clamp_to_zero_with_larger_uint_max!(i16 => u16, u32, u64, u128, usize);
impl_int_clamp_to_smaller!(i16 => u8);

// i32
impl_lossless_casts!(i32 => i32, i64, i128);
impl_int_clamp_to_smaller!(i32 => i8, i16);
impl_int_clamp_to_zero_with_larger_uint_max!(i32 => u32, u64, u128);
impl_int_clamp_to_smaller!(i32 => u8, u16);
impl_int_isize_usize!(i32);

// i64
impl_lossless_casts!(i64 => i64, i128);
impl_int_clamp_to_smaller!(i64 => i8, i16, i32);
impl_int_clamp_to_zero_with_larger_uint_max!(i64 => u64, u128);
impl_int_clamp_to_smaller!(i64 => u8, u16, u32);
impl_int_isize_usize!(i64);

// i128
impl_lossless_casts!(i128 => i128);
impl_int_clamp_to_smaller!(i128 => i8, i16, i32, i64);
impl_int_clamp_to_zero_with_larger_uint_max!(i128 => u128);
impl_int_clamp_to_smaller!(i128 => u8, u16, u32, u64);
impl_int_isize_usize!(i128);

// isize
impl_lossless_casts!(isize => isize);
impl_int_clamp_to_smaller!(isize => i8, i16);
impl_isize_casts!(i32, i64, i128);
impl_int_clamp_to_zero_with_larger_uint_max!(isize => u64, u128, usize);
impl_int_clamp_to_smaller!(isize => u8, u16);
impl_isize_casts!(u32);

#[cfg(test)]
mod casts {
    use crate::SaturatingCast;

    use core::{i128, i16, i32, i64, i8, isize};
    use core::{u128, u16, u32, u64, u8, usize};

    macro_rules! impl_test_all_casts {
        ($src: ty => $($target: ty),*) => {$(
            let _: $target = <$src>::MIN.saturating_cast::<$target>();
        )*};
    }

    #[test]
    fn u8_all_casts() {
        impl_test_all_casts!(u8 => u8, u16, u32, u64, u128, usize);
        impl_test_all_casts!(u8 => i8, i16, i32, i64, i128, isize);
    }

    #[test]
    fn u16_all_casts() {
        impl_test_all_casts!(u16 => u8, u16, u32, u64, u128, usize);
        impl_test_all_casts!(u16 => i8, i16, i32, i64, i128, isize);
    }

    #[test]
    fn u32_all_casts() {
        impl_test_all_casts!(u32 => u8, u16, u32, u64, u128, usize);
        impl_test_all_casts!(u32 => i8, i16, i32, i64, i128, isize);
    }

    #[test]
    fn u64_all_casts() {
        impl_test_all_casts!(u64 => u8, u16, u32, u64, u128, usize);
        impl_test_all_casts!(u64 => i8, i16, i32, i64, i128, isize);
    }

    #[test]
    fn u128_all_casts() {
        impl_test_all_casts!(u128 => u8, u16, u32, u64, u128, usize);
        impl_test_all_casts!(u128 => i8, i16, i32, i64, i128, isize);
    }

    #[test]
    fn usize_all_casts() {
        impl_test_all_casts!(usize => u8, u16, u32, u64, u128, usize);
        impl_test_all_casts!(usize => i8, i16, i32, i64, i128, isize);
    }

    #[test]
    fn i8_all_casts() {
        impl_test_all_casts!(i8 => u8, u16, u32, u64, u128, usize);
        impl_test_all_casts!(i8 => i8, i16, i32, i64, i128, isize);
    }

    #[test]
    fn i16_all_casts() {
        impl_test_all_casts!(i16 => u8, u16, u32, u64, u128, usize);
        impl_test_all_casts!(i16 => i8, i16, i32, i64, i128, isize);
    }

    #[test]
    fn i32_all_casts() {
        impl_test_all_casts!(i32 => u8, u16, u32, u64, u128, usize);
        impl_test_all_casts!(i32 => i8, i16, i32, i64, i128, isize);
    }

    #[test]
    fn i64_all_casts() {
        impl_test_all_casts!(i64 => u8, u16, u32, u64, u128, usize);
        impl_test_all_casts!(i64 => i8, i16, i32, i64, i128, isize);
    }

    #[test]
    fn i128_all_casts() {
        impl_test_all_casts!(i128 => u8, u16, u32, u64, u128, usize);
        impl_test_all_casts!(i128 => i8, i16, i32, i64, i128, isize);
    }

    #[test]
    fn isize_all_casts() {
        impl_test_all_casts!(isize => u8, u16, u32, u64, u128, usize);
        impl_test_all_casts!(isize => i8, i16, i32, i64, i128, isize);
    }
}
