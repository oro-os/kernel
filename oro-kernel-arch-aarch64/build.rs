#![expect(missing_docs, clippy::missing_docs_in_private_items)]

fn main() {
	println!("cargo:rustc-link-arg-bin=oro-kernel-aarch64=-T");
	println!("cargo:rustc-link-arg-bin=oro-kernel-aarch64=oro-kernel-arch-aarch64/arch.x");
	println!("cargo:rerun-if-changed=oro-kernel-arch-aarch64/arch.x");
}
