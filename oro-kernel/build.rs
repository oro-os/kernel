fn main() {
	#[cfg(target_arch = "x86_64")]
	{
		println!("cargo:rustc-link-arg-bin=oro-kernel=-T");
		println!("cargo:rustc-link-arg-bin=oro-kernel=oro-kernel/link/x86_64.ld",);
	}
}
