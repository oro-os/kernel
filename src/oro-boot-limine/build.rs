fn main() {
	let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();
	match target_arch.as_str() {
		"x86_64" => {
			println!("cargo:rustc-link-arg-bin=oro-boot-limine=-T");
			println!("cargo:rustc-link-arg-bin=oro-boot-limine=src/oro-boot-limine/link/x86_64.ld",);
		}
		target_arch => {
			panic!("unknown or unsupported architecture: {target_arch}");
		}
	}
}
