// Copyright © 2026, Julian Scheffers
// SPDX-License-Identifier: MIT OR Apache-2.0

/// Executable entrypoint function signature.
pub type EntryPoint = unsafe extern "C" fn() -> !;
