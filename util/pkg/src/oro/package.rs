//! The `oro.package` module; buildable package definitions.

use koto::{derive::*, prelude::*, runtime::Result};

/// How a package's output artifact is produced.
#[derive(Clone)]
pub enum PackageOutput {
	/// A single output file with the given extension.
	///
	/// The thunk is deferred until the package is built, at which point
	/// it is called with the output artifact's path and is responsible
	/// for producing it (e.g. via an `oro.iso` builder chain's `build`).
	SingleFile { extension: String, thunk: KValue },
}

/// A buildable package definition, produced by package export functions
/// in the `pkg/platform` scripts.
#[derive(Clone, Default, KotoType, KotoCopy)]
pub struct Package {
	/// The architectures the package targets.
	pub archs: Vec<String>,
	pub output: Option<PackageOutput>,
}

#[koto_impl(runtime = koto::runtime)]
impl Package {
	/// Sets the architectures the package targets.
	#[koto_method]
	fn arch(ctx: MethodContext<Self>) -> Result<KValue> {
		ctx.instance_mut()?.archs = parse_archs(ctx.args)?;
		ctx.instance_result()
	}

	/// Declares that the package produces a single output file with the
	/// given extension, built by the given function.
	#[koto_method]
	fn single_file(ctx: MethodContext<Self>) -> Result<KValue> {
		match ctx.args {
			[KValue::Str(extension), thunk] if thunk.is_callable() => {
				ctx.instance_mut()?.output = Some(PackageOutput::SingleFile {
					extension: extension.to_string(),
					thunk: thunk.clone(),
				});
				ctx.instance_result()
			}
			unexpected => unexpected_args("|String, Callable|", unexpected),
		}
	}
}

impl KotoObject for Package {
	fn display(&self, ctx: &mut DisplayContext) -> Result<()> {
		ctx.append(format!("package(archs: {})", self.archs.join(", ")));
		Ok(())
	}
}

fn parse_archs(args: &[KValue]) -> Result<Vec<String>> {
	let mut archs = Vec::with_capacity(args.len());
	for arg in args {
		match arg {
			KValue::Str(arch) => archs.push(arch.to_string()),
			_ => return unexpected_args("|String...|", args),
		}
	}
	Ok(archs)
}

/// Creates the `oro.package` module.
pub fn make_module() -> KMap {
	let map = KMap::with_type("package");

	// `package.arch ...` begins a new package definition.
	map.add_fn("arch", |ctx| {
		let package = Package {
			archs: parse_archs(ctx.args())?,
			output: None,
		};
		Ok(KValue::Object(KObject::from(package)))
	});

	map
}
