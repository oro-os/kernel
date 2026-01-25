#![expect(missing_docs)]

fn main() {
	let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
	println!("cargo::rustc-link-search=native={}", manifest_dir);
}
