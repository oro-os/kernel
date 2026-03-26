#[doc(hidden)]
macro_rules! impl_type {
    (for $arch:ty {
        $(
			$(#[$attr:meta])*
			use $base_ty:ident :: $assoc_ty:ident as $trait_ty:ident
		);* $(;)?
    }) => {
        $(
			$(#[$attr])*
			#[doc = concat!(
				"Re-export of [`orok_arch_base::",
				::core::stringify!($base_ty),
				"::",
				::core::stringify!($assoc_ty),
				"`]"
			)]
            pub type $trait_ty = impl orok_arch_base::$trait_ty;
            const _: () = {
                #[doc(hidden)]
                #[define_opaque($trait_ty)]
                const fn _assoc_ty(_: $arch) -> $trait_ty {
                    let u = ::core::mem::MaybeUninit::<<$arch as orok_arch_base::$base_ty>::$assoc_ty>::uninit();
					// SAFETY: Never actually called.
                    unsafe { u.assume_init() }
                }
            };
        )*
    };
}

#[doc(hidden)]
macro_rules! impl_fn {
	(for $arch:ty {
		$(
			$(#[$attr:meta])*
			unsafe fn $base_ty:ident :: $assoc_fn:ident as $fn_name:ident(
				$($argname:ident: $argty:ty),* $(,)?
			) $(-> $retty:ty)?);* $(;)?
	}) => {
		$(
			#[inline]
			#[doc = concat!(
				"Re-export of [`orok_arch_base::",
				::core::stringify!($base_ty),
				"::",
				::core::stringify!($assoc_fn),
				"`]"
			)]
			///
			/// # Safety
			/// See the base function (above) for safety information.
			$(#[$attr])*
			pub unsafe fn $fn_name($($argname: $argty),*) $(-> $retty)? {
			// SAFETY: Unsafe requirements offloaded to caller.
			unsafe {
				<$arch as orok_arch_base::$base_ty>::$assoc_fn($($argname)*)
			}
		})*
	};

	(for $arch:ty {
		$(
			$(#[$attr:meta])*
			fn $base_ty:ident :: $assoc_fn:ident as $fn_name:ident(
				$($argname:ident: $argty:ty),* $(,)?
			) $(-> $retty:ty)?);* $(;)?
	}) => {
		$(
			#[inline]
			#[doc = concat!(
				"Re-export of [`orok_arch_base::",
				::core::stringify!($base_ty),
				"::",
				::core::stringify!($assoc_fn),
				"`]"
			)]
			$(#[$attr])*
			pub fn $fn_name($($argname: $argty),*) $(-> $retty)? {
			<$arch as orok_arch_base::$base_ty>::$assoc_fn($($argname)*)
			}
		)*
	};
}
