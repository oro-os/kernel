macro_rules! ok {
	($($arg:tt)*) => ($crate::arch::print_args(format_args!("ok::{}\n", format_args!($($arg)*))));
}

pub(crate) use ok;

macro_rules! warning {
	($($arg:tt)*) => ($crate::arch::print_args(format_args!("warn::{}\n", format_args!($($arg)*))));
}

pub(crate) use warning;

macro_rules! debug {
	($($arg:tt)*) => ($crate::arch::print_args(format_args!("debug::{}\n", format_args!($($arg)*))));
}

pub(crate) use debug;

macro_rules! kernel_panic {
	($($arg:tt)*) => ($crate::arch::print_args(format_args!("panic::{}\n", format_args!($($arg)*))));
}

pub(crate) use kernel_panic;
