//! Provides debuggers with a few special utilities for helping
//! with debugging. These are incredibly unsafe and should only
//! be used in a controlled environment under a debugger.

#[cfg(any(debug_assertions, feature = "dbgutil"))]
use oro_common_proc::gdb_autoload_inline;

#[cfg(any(debug_assertions, feature = "dbgutil"))]
gdb_autoload_inline!("dbgutil.py");
