fn main() {
	let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();
	assert_eq!(target_arch, "x86_64");
	println!("cargo:rustc-link-arg-bin=oro-boot-limine-x64=-T");
	println!("cargo:rustc-link-arg-bin=oro-boot-limine-x64=oro-boot-limine-x64/link.ld",);
}
