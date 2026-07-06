use crate::vfs::Lockfile;

#[expect(unreachable_code)]
pub fn run() {
	panic!("command is disabled until `cargo vendor` is fixed: https://github.com/rust-lang/cargo/issues/7058");

	let vfs = crate::vfs::Vfs::new_from_cargo();
	let lockfile = vfs.lockfile().collect::<Vec<_>>();

	let mut found = false;
	for dep in lockfile.unused() {
		eprintln!("unused dependency: {}", dep.package_name);
		found = true;
	}

	if found {
		std::process::exit(1);
	}

	eprintln!("no unused dependencies found");
}
