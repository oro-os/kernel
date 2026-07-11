#![expect(missing_docs)]

fn main() {
	println!("cargo:rustc-link-arg-bin=oro-kernel-x86_64=-Tartifact/x86_64-kernel/arch.x",);
	println!("cargo:rerun-if-changed=artifact/x86_64-kernel/arch.x");
}
