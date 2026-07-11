#![expect(missing_docs)]

fn main() {
	println!("cargo:rustc-link-arg-bin=oro-kernel-aarch64=-Tartifact/aarch64-kernel/arch.x",);
	println!("cargo:rerun-if-changed=artifact/aarch64-kernel/arch.x");
}
