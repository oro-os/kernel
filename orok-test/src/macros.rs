#[macro_export]
macro_rules! dbgstr {
	($($tt:tt)+) => {{
		const DBG_STR: &str = concat!($($tt)+, "\0");
		const DBG_LEN: usize = DBG_STR.len();

		const fn str_to_array<const N: usize>(s: &str) -> [u8; N] {
			let mut arr = [0u8; N];
			let bytes = s.as_bytes();
			let mut i = 0usize;
			while i < N {
				arr[i] = bytes[i];
				i += 1;
			}
			arr
		}

		#[unsafe(link_section = ".orok_test_strings")]
		#[used]
		static DBG_BYTES: [u8; DBG_LEN] = str_to_array(DBG_STR);

		(&raw const DBG_BYTES).addr()
	}};
}

#[macro_export]
#[cfg(feature = "mmio")]
macro_rules! emit_raw {
	($r0:expr, $r1:expr, $r2:expr, $r3:expr, $r4:expr, $r5:expr, $r6:expr, $r7:expr $(,)?) => {
		// SAFETY: This is safe as we are only emitting these instructions
		// SAFETY: in test builds and under very controlled environments.
		unsafe {
			#[cfg(target_arch = "x86_64")]
			let regs = &mut *(($crate::get_vmm_base() + 0xFEB00000) as *mut [u64; 8]);
			#[cfg(target_arch = "aarch64")]
			let regs = &mut *(($crate::get_vmm_base() + 0x090D0000) as *mut [u64; 8]);
			#[cfg(target_arch = "riscv64")]
			let regs = &mut *(($crate::get_vmm_base() + 0x10002000) as *mut [u64; 8]);
			#[cfg(not(any(
				target_arch = "x86_64",
				target_arch = "aarch64",
				target_arch = "riscv64"
			)))]
			compile_error!("unsupported architecture");

			// Always write r0 last, as it causes the actual emission
			// to the event stream.
			::core::ptr::write_volatile(&mut regs[7], $r7 as u64);
			::core::ptr::write_volatile(&mut regs[6], $r6 as u64);
			::core::ptr::write_volatile(&mut regs[5], $r5 as u64);
			::core::ptr::write_volatile(&mut regs[4], $r4 as u64);
			::core::ptr::write_volatile(&mut regs[3], $r3 as u64);
			::core::ptr::write_volatile(&mut regs[2], $r2 as u64);
			::core::ptr::write_volatile(&mut regs[1], $r1 as u64);
			::core::sync::atomic::compiler_fence(::core::sync::atomic::Ordering::SeqCst);
			::core::ptr::write_volatile(&mut regs[0], $r0 as u64);
			::core::sync::atomic::compiler_fence(::core::sync::atomic::Ordering::SeqCst);
		}
	};
}

#[macro_export]
#[cfg(feature = "emit")]
macro_rules! emit {
	($id:expr, $r1:expr, $r2:expr, $r3:expr, $r4:expr, $r5:expr, $r6:expr, $r7:expr $(,)?) => {{
		const _: () = const {
			assert!(
				($id & 0xFFFF_0000_0000_0000) == 0,
				"orok-test event ID cannot have high 16 bits set"
			);
		};
		$crate::emit_raw!($id, $r1, $r2, $r3, $r4, $r5, $r6, $r7);
	}};
	($id:expr, $r1:expr, $r2:expr, $r3:expr, $r4:expr, $r5:expr, $r6:expr $(,)?) => {
		$crate::emit!($id, $r1, $r2, $r3, $r4, $r5, $r6, 0)
	};
	($id:expr, $r1:expr, $r2:expr, $r3:expr, $r4:expr, $r5:expr $(,)?) => {
		$crate::emit!($id, $r1, $r2, $r3, $r4, $r5, 0, 0)
	};
	($id:expr, $r1:expr, $r2:expr, $r3:expr, $r4:expr $(,)?) => {
		$crate::emit!($id, $r1, $r2, $r3, $r4, 0, 0, 0)
	};
	($id:expr, $r1:expr, $r2:expr, $r3:expr $(,)?) => {
		$crate::emit!($id, $r1, $r2, $r3, 0, 0, 0, 0)
	};
	($id:expr, $r1:expr, $r2:expr $(,)?) => {
		$crate::emit!($id, $r1, $r2, 0, 0, 0, 0, 0)
	};
	($id:expr, $r1:expr $(,)?) => {
		$crate::emit!($id, $r1, 0, 0, 0, 0, 0, 0)
	};
	($id:expr $(,)?) => {
		$crate::emit!($id, 0, 0, 0, 0, 0, 0, 0)
	};
}

#[macro_export]
#[cfg(not(feature = "emit"))]
macro_rules! emit {
	($($tt:tt)*) => {};
}

#[macro_export]
#[cfg(feature = "emit")]
macro_rules! emit_effect {
	( $id:expr, $loc:expr, write_reg = cr0 ) => {
		$crate::emit!($id, $loc, $crate::consts::X8664_EFFECT_WRITE_REG_CR0);
	};
	( $id:expr, $loc:expr, read_reg = cr0 ) => {
		$crate::emit!($id, $loc, $crate::consts::X8664_EFFECT_READ_REG_CR0);
	};
	( $( $tt:tt )+ ) => {
		compile_error!(concat!("unknown effect annotation: ",  $( stringify!($tt) ),+ ))
	};
}

#[macro_export]
#[cfg(feature = "emit")]
macro_rules! annotate_effect_fn {
	( start @ $fn_name:literal => { $($tt:tt)* } ) => {{
		$crate::emit_effect!(
			$crate::consts::EFFECT_START,
			$crate::dbgstr!(::core::file!(), ":", ::core::line!(), ": ", ::core::module_path!(), "::", $fn_name),
			$($tt)*
		);
	}};

	( end @ $fn_name:literal => { $($tt:tt)* } ) => {{
		$crate::emit_effect!(
			$crate::consts::EFFECT_END,
			$crate::dbgstr!(::core::file!(), ":", ::core::line!(), ": ", ::core::module_path!(), "::", $fn_name),
			$($tt)*
		);
	}};
}

#[macro_export]
#[cfg(not(feature = "emit"))]
macro_rules! annotate_effect_fn {
	($($tt:tt)*) => {};
}

#[macro_export]
#[cfg(feature = "emit")]
macro_rules! oro_has_started_execution {
	() => {{
		$crate::emit!(
			$crate::consts::IN_KERNEL,
			$crate::dbgstr!(
				::core::file!(),
				":",
				::core::line!(),
				": ",
				::core::module_path!()
			),
		);
	}};
}

#[macro_export]
#[cfg(not(feature = "emit"))]
macro_rules! oro_has_started_execution {
	($($tt:tt)*) => {};
}
