fn main() {
	let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();

	match target_arch.as_str() {
		"x86_64" => {
			println!("cargo:rustc-link-arg-bin=oro-kernel-x86_64=-T");
			println!("cargo:rustc-link-arg-bin=oro-kernel-x86_64=oro-kernel/bin/x86_64.ld",);
			println!("cargo:rerun-if-changed=oro-kernel/bin/x86_64.ld");
		}
		"aarch64" => {
			println!("cargo:rustc-link-arg-bin=oro-kernel-aarch64=-T");
			println!("cargo:rustc-link-arg-bin=oro-kernel-aarch64=oro-kernel/bin/aarch64.ld",);
			println!("cargo:rerun-if-changed=oro-kernel/bin/aarch64.ld");
		}
		_ => {
			panic!("unsupported target architecture: {target_arch}");
		}
	}
}
