#![expect(missing_docs)]

fn main() {
	println!("cargo:rustc-link-arg-bin=oro-limine-x86-64=-Tartifact/x86_64-limine/arch.x",);
	println!("cargo:rerun-if-changed=artifact/x86_64-limine/arch.x");
}
