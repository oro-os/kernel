//! Provides debuggers with a few special utilities for helping
//! with debugging. These are incredibly unsafe and should only
//! be used in a controlled environment under a debugger.

#[cfg(not(debug_assertions))]
compile_error!("The `dbgutil` module should only be used in debug builds.");

#[cfg(feature = "dbgutil")]
use oro_common_proc::gdb_autoload_inline;

#[cfg(feature = "dbgutil")]
gdb_autoload_inline!("dbgutil.py");
