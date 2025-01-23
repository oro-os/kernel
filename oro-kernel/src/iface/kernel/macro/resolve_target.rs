// NOTE(qix-): This is NOT a module. It's meant to be `include!`d in another module.


// Resolves the target thread from the given index,
// checking that the caller has permission to access it.
#[doc(hidden)]
macro_rules! resolve_target {
	($thread:expr, $index:expr) => {{
		let thread = $thread;
		let index = $index;
		if index == 0 || index == thread.id() {
			thread.clone()
		} else {
			match crate::tab::get().lookup::<Thread<A>>(index) {
				Some(t) => {
					if t.with(|t| t.ring().id()) != thread.with(|t| t.ring().id()) {
						return InterfaceResponse::immediate(SysError::BadIndex, 0);
					}

					t
				}
				None => {
					return InterfaceResponse::immediate(SysError::BadIndex, 0);
				}
			}
		}
	}};
}
