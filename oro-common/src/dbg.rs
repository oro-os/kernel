#![allow(unused_macros)]

/// Sends a general debug message to the archiecture-specific debug endpoint.
#[macro_export]
macro_rules! dbg {
	($Arch:ty, $tag:literal, $($arg:tt)*) => {
		{
			<$Arch>::log(format_args!(" :{}:{}", $tag, format_args!($($arg)*)));
		}
	}
}

/// Sends an error debug message to the archiecture-specific debug endpoint.
#[macro_export]
macro_rules! dbg_err {
	($Arch:ty, $tag:literal, $($arg:tt)*) => {
		{
			<$Arch>::log(format_args!("E:{}:{}", $tag, format_args!($($arg)*)));
		}
	}
}

/// Sends an error debug message to the archiecture-specific debug endpoint.
#[macro_export]
macro_rules! dbg_warn {
	($Arch:ty, $tag:literal, $($arg:tt)*) => {
		{
			<$Arch>::log(format_args!("W:{}:{}", $tag, format_args!($($arg)*)));
		}
	}
}
