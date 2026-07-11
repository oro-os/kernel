#![expect(missing_docs)]

fn main() {
	println!("cargo:rustc-link-arg-bin=oro-limine-riscv64=-Tartifact/riscv64-limine/arch.x",);
	println!("cargo:rerun-if-changed=artifact/riscv64-limine/arch.x");
}
