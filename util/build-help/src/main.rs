#[cfg(not(any(test, doc, miri)))]
compile_error!("run `cargo oro --help` instead");

fn main() {
	panic!("not sure how you built this, but this does nothing; run `cargo oro --help` instead");
}
