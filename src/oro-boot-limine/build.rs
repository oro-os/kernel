fn main() {
	#[cfg(target_arch = "x86_64")]
	{
		println!("cargo:rustc-link-arg-bin=oro-boot-limine=-T");
		println!("cargo:rustc-link-arg-bin=oro-boot-limine=src/oro-boot-limine/link/x86_64.ld",);
	}
}
