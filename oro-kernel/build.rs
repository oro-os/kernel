#![allow(missing_docs, clippy::missing_docs_in_private_items)]

fn main() {
	let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();

	match target_arch.as_str() {
		"x86_64" => {
			println!("cargo:rustc-link-arg-bin=oro-kernel-x86_64=-T");
			println!("cargo:rustc-link-arg-bin=oro-kernel-x86_64=oro-kernel/bin/x86_64.x",);
			println!("cargo:rerun-if-changed=oro-kernel/bin/x86_64.x");
		}
		"aarch64" => {
			println!("cargo:rustc-link-arg-bin=oro-kernel-aarch64=-T");
			println!("cargo:rustc-link-arg-bin=oro-kernel-aarch64=oro-kernel/bin/aarch64.x",);
			println!("cargo:rerun-if-changed=oro-kernel/bin/aarch64.x");
		}
		_ => {
			panic!("unsupported target architecture: {target_arch}");
		}
	}
}
