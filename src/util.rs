#[doc(hidden)]
#[macro_export]
macro_rules! static_assert {
	($cond:expr) => {
		const _: () = ::core::assert!($cond);
	};
}
