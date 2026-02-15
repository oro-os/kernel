//! Tests for the `bitstruct!` macro.

use orok_macro::bitstruct;

#[test]
fn empty_u64() {
	bitstruct! {
		struct Empty(u64) {}
	}

	let v = Empty(1337u64);
	assert_eq!(v.0, 1337u64);
}

#[test]
fn empty_u32() {
	bitstruct! {
		struct Empty(u32) {}
	}

	let v = Empty(1337u32);
	assert_eq!(v.0, 1337u32);
}

#[test]
fn empty_u16() {
	bitstruct! {
		struct Empty(u16) {}
	}

	let v = Empty(1337u16);
	assert_eq!(v.0, 1337u16);
}

#[test]
fn empty_u8() {
	bitstruct! {
		struct Empty(u8) {}
	}

	let v = Empty(42u8);
	assert_eq!(v.0, 42u8);
}

#[test]
fn preserve_unspecified() {
	bitstruct! {
		struct Reg(u64) {
			pub a[0] => as bool,
			pub b[1] => as bool,
			pub c[2] => as bool,
		}
	}

	let mut v = Reg(0xAABB_CCDD_EEFF_1234u64);
	assert_eq!(v.0, 0xAABB_CCDD_EEFF_1234u64);
	assert_eq!(v.a(), false);
	assert_eq!(v.b(), false);
	assert_eq!(v.c(), true);
	v.set_a(true);
	assert_eq!(v.0, 0xAABB_CCDD_EEFF_1234u64 | 0b1);
	assert_eq!(v.a(), true);
	assert_eq!(v.b(), false);
	assert_eq!(v.c(), true);
	v.set_b(true);
	assert_eq!(v.0, 0xAABB_CCDD_EEFF_1234 | 0b11);
	assert_eq!(v.a(), true);
	assert_eq!(v.b(), true);
	assert_eq!(v.c(), true);
	v.set_c(false);
	assert_eq!(
		v.0,
		(0xAABB_CCDD_EEFF_1234u64 & 0xFFFF_FFFF_FFFF_FFF8u64) | 0b11
	);
	assert_eq!(v.a(), true);
	assert_eq!(v.b(), true);
	assert_eq!(v.c(), false);
}

#[test]
fn default_with_const() {
	bitstruct! {
		struct Reg(u64) {
			[7:3] => 0b10101,
		}
	}

	let v = Reg::default();
	assert_eq!(v.0, 0b10101 << 3);
}

#[test]
fn non_exhaustive_non_on_invalid() {
	bitstruct! {
		struct Reg(u64) {
			#[non_exhaustive]
			pub foo[3:0] => enum MyFoo(u8) {
				Bar = 0b0101,
				Baz = 0b1010,
				Qux = 0b1111,
			}
		}
	}

	let v = Reg(0b1010);
	assert_eq!(v.foo(), Some(MyFoo::Baz));
	let v = Reg(0b0101);
	assert_eq!(v.foo(), Some(MyFoo::Bar));
	let v = Reg(0b1111);
	assert_eq!(v.foo(), Some(MyFoo::Qux));
	let v = Reg(0b0000);
	assert_eq!(v.foo(), None);
	let v = Reg(0b1101);
	assert_eq!(v.foo(), None);
}
