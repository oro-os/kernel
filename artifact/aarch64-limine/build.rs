#![expect(missing_docs)]

fn main() {
	println!("cargo:rustc-link-arg-bin=oro-limine-aarch64=-Tartifact/aarch64-limine/arch.x",);
	println!("cargo:rerun-if-changed=artifact/aarch64-limine/arch.x");
}
