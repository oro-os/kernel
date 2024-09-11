#![expect(missing_docs, clippy::missing_docs_in_private_items)]

fn main() {
	println!("cargo:rustc-link-arg-bin=oro-kernel-x86_64=-T");
	println!("cargo:rustc-link-arg-bin=oro-kernel-x86_64=oro-arch-x86_64/arch.x");
	println!("cargo:rerun-if-changed=oro-kernel-x86_64/arch.x");
}
