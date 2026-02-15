#![expect(missing_docs, reason = "build scripts don't need docs")]
#![expect(
	clippy::unwrap_used,
	reason = "build scripts are allowed to panic if environment variables are missing, as this is \
	          a build-time error that should be caught during development"
)]

fn main() {
	let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
	println!("cargo::rustc-link-search=native={manifest_dir}");
}
