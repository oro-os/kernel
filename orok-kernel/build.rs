#![expect(missing_docs, reason = "build scripts don't need docs")]
#![expect(
	clippy::unwrap_used,
	clippy::panic,
	reason = "build scripts are allowed to panic if environment variables are missing, as this is \
	          a build-time error that should be caught during development"
)]

fn main() {
	let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();

	println!("cargo::rustc-link-arg=-Torok-test.x");

	match target_arch.as_str() {
		"x86_64" => {
			println!("cargo:rustc-link-arg-bin=oro-kernel=-Torok-kernel/x86_64.x");
			println!("cargo:rerun-if-changed=orok-kernel/x86_64.x");
		}
		"aarch64" => {
			println!("cargo:rustc-link-arg-bin=oro-kernel=-Torok-kernel/aarch64.x");
			println!("cargo:rerun-if-changed=orok-kernel/aarch64.x");
		}
		"riscv64" => {
			println!("cargo:rustc-link-arg-bin=oro-kernel=-Torok-kernel/riscv64.x");
			println!("cargo:rerun-if-changed=orok-kernel/riscv64.x");
		}
		_ => {
			panic!("unsupported target architecture: {target_arch}");
		}
	}
}
