#![expect(missing_docs)]
fn main() {
	// Allow builds if:
	// 1. The "run-from-cargo" feature is enabled (for development)
	// 2. The ORO_BUILD_TOOL environment variable is set (when invoked by cargo oro)
	if cfg!(not(feature = "run-from-cargo")) && std::env::var("ORO_BUILD_TOOL").is_err() {
		panic!(
			"This workspace must be built with `cargo oro build`. See `cargo oro --help` for \
			 usage."
		);
	}
}
