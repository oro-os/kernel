#![expect(missing_docs)]

fn main() {
	println!("cargo:rustc-link-arg-bin=oro-kernel-riscv64=-Tartifact/riscv64-kernel/arch.x",);
	println!("cargo:rerun-if-changed=artifact/riscv64-kernel/arch.x");
}
