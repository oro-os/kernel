//! The `oro.file` module; file references for package specifications.

use std::path::{Path, PathBuf};

use koto::{
	derive::*,
	prelude::*,
	runtime::{Ptr, Result},
};

/// The source of a [`File`]'s contents.
#[derive(Clone, Debug)]
pub enum FileSource {
	/// A file on disk.
	///
	/// The path is not required to exist until a package referencing
	/// it is built (e.g. kernel binaries that haven't been built yet).
	Disk(PathBuf),
	/// An in-memory file, staged to disk when a package referencing it
	/// is built.
	Memory(Vec<u8>),
	/// A file whose contents are produced on demand, when a package
	/// referencing it is built (e.g. a ramdisk encoded from other
	/// files).
	///
	/// Like [`FileSource::Disk`], nothing is read or computed until
	/// then; a generator over artifacts that haven't been built yet is
	/// perfectly valid until something asks for its contents.
	Generated(Ptr<dyn GenerateFile>),
}

/// Produces a [`File`]'s contents on demand.
pub trait GenerateFile: std::fmt::Debug {
	/// Produces the contents, or a human-readable reason it could not
	/// be produced.
	fn generate(&self) -> std::result::Result<Vec<u8>, String>;
}

/// A reference to a file that can be placed into a package.
#[derive(Clone, Debug, KotoType, KotoCopy)]
pub struct File {
	pub source: FileSource,
}

impl File {
	pub fn disk(path: impl Into<PathBuf>) -> Self {
		Self {
			source: FileSource::Disk(path.into()),
		}
	}

	pub fn memory(contents: impl Into<Vec<u8>>) -> Self {
		Self {
			source: FileSource::Memory(contents.into()),
		}
	}

	pub fn generated(generator: impl GenerateFile + 'static) -> Self {
		Self {
			source: FileSource::Generated(Ptr::from(Box::new(generator) as Box<dyn GenerateFile>)),
		}
	}
}

impl KotoEntries for File {}

impl KotoObject for File {
	fn display(&self, ctx: &mut DisplayContext) -> Result<()> {
		match &self.source {
			FileSource::Disk(path) => ctx.append(format!("file({})", path.display())),
			FileSource::Memory(contents) => {
				ctx.append(format!("file(<memory: {} bytes>)", contents.len()));
			}
			FileSource::Generated(generator) => {
				ctx.append(format!("file(<generated: {generator:?}>)"));
			}
		}
		Ok(())
	}
}

impl From<File> for KValue {
	fn from(file: File) -> Self {
		KValue::Object(KObject::from(file))
	}
}

/// Creates the `oro.file` module.
///
/// The module itself is callable (`file './some/path'`), resolving
/// relative paths against the directory of the calling script.
pub fn make_module() -> KMap {
	let mut map = KMap::with_type("file");

	map.add_fn("from_str", |ctx| {
		match ctx.args() {
			[KValue::Str(contents)] => Ok(File::memory(contents.as_bytes()).into()),
			unexpected => unexpected_args("|String|", unexpected),
		}
	});

	map.insert_meta(
		MetaKey::Call,
		KValue::NativeFunction(KNativeFunction::new(|ctx: &mut CallContext| {
			match ctx.args() {
				[KValue::Str(path)] => {
					let path = Path::new(path.as_str());
					let path = if path.is_absolute() {
						path.to_path_buf()
					} else {
						let chunk = ctx.vm.chunk();
						let Some(script_dir) = chunk
							.path
							.as_ref()
							.and_then(|p| Path::new(p.as_str()).parent())
						else {
							return runtime_error!(
								"cannot resolve relative file path '{}': calling script has no \
								 path",
								path.display()
							);
						};
						script_dir.join(path)
					};
					Ok(File::disk(path).into())
				}
				unexpected => unexpected_args("|String|", unexpected),
			}
		})),
	);

	map
}
