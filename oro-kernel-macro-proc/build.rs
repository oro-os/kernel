#![expect(missing_docs, clippy::missing_docs_in_private_items)]

fn main() {
	println!("cargo:out_dir={}", std::env::var("OUT_DIR").unwrap());
}
