//! The `oro.ramdisk` module; declarative Oro ramdisk specifications,
//! backed by [`oro_ramdisk`]'s encoder at build time.

use std::io::{Read, Seek, Write};

use koto::{derive::*, prelude::*, runtime::Result as KotoResult};
use oro_ramdisk::{
	Encoder,
	item::{KernelArch, KernelEncoder},
};

use super::file::{File, FileSource, GenerateFile};

/// The zstd level used by `compress` when the script names none.
pub const DEFAULT_COMPRESSION_LEVEL: i32 = 3;

/// The zstd levels the encoder accepts; outside this range
/// `KernelEncoder::new_compress_with_zstd` panics, so scripts are
/// rejected before they get there.
pub const COMPRESSION_LEVELS: std::ops::RangeInclusive<i32> = -7..=22;

/// A single item to be encoded into a ramdisk.
///
/// One variant per item type the ramdisk format defines; adding a new
/// item type is a variant here, a constructor in [`make_module`], and
/// an arm in [`RamdiskBuilder::encode_to`].
#[derive(Clone, Debug)]
pub enum ItemSpec {
	/// A `KERN` item: the kernel image for a single architecture.
	Kernel {
		file: File,
		arch: KernelArch,
		/// The zstd compression level, or `None` to store the image
		/// uncompressed.
		compression: Option<i32>,
	},
}

/// A single [`ItemSpec`] as handed to Koto, so that item options can
/// be chained onto it before it is added to a builder.
#[derive(Clone, Debug, KotoType, KotoCopy)]
pub struct Item(pub ItemSpec);

#[koto_impl(runtime = koto::runtime)]
impl Item {
	/// Compresses the item's payload with zstd, at the given level or
	/// [`DEFAULT_COMPRESSION_LEVEL`] if none is given.
	#[koto_method]
	fn compress(ctx: MethodContext<Self>) -> KotoResult<KValue> {
		let level = match ctx.args {
			[] => DEFAULT_COMPRESSION_LEVEL,
			[KValue::Number(level)] => i32::from(level),
			unexpected => return unexpected_args("|| or |Number|", unexpected),
		};

		if !COMPRESSION_LEVELS.contains(&level) {
			return runtime_error!(
				"compression level {level} is out of range (expected {} to {})",
				COMPRESSION_LEVELS.start(),
				COMPRESSION_LEVELS.end()
			);
		}

		match &mut ctx.instance_mut()?.0 {
			ItemSpec::Kernel { compression, .. } => *compression = Some(level),
		}

		ctx.instance_result()
	}
}

impl KotoObject for Item {
	fn display(&self, ctx: &mut DisplayContext) -> KotoResult<()> {
		match &self.0 {
			ItemSpec::Kernel { arch, .. } => ctx.append(format!("ramdisk.kernel({arch:?})")),
		}
		Ok(())
	}
}

/// A declarative ramdisk specification, built up via `ramdisk.builder()`
/// and consumed by `.file()` or `.build`.
#[derive(Clone, Debug, Default, KotoType, KotoCopy)]
pub struct RamdiskBuilder {
	pub items: Vec<ItemSpec>,
}

#[koto_impl(runtime = koto::runtime)]
impl RamdiskBuilder {
	/// Appends an item to the ramdisk.
	#[koto_method]
	fn item(ctx: MethodContext<Self>) -> KotoResult<KValue> {
		match ctx.args {
			[KValue::Object(item)] => {
				let spec = item.cast::<Item>()?.0.clone();
				ctx.instance_mut()?.items.push(spec);
				ctx.instance_result()
			}
			unexpected => unexpected_args("|Item|", unexpected),
		}
	}

	/// Returns the ramdisk as a lazily-encoded `File`.
	#[koto_method]
	fn file(ctx: MethodContext<Self>) -> KotoResult<KValue> {
		match ctx.args {
			[] => Ok(ctx.instance()?.to_file().into()),
			unexpected => unexpected_args("||", unexpected),
		}
	}

	/// Writes the ramdisk to the given destination, returning the path.
	#[koto_method]
	fn build(ctx: MethodContext<Self>) -> KotoResult<KValue> {
		match ctx.args {
			[KValue::Str(dest)] => {
				let spec = ctx.instance()?.clone();
				match spec.build_to(std::path::Path::new(dest.as_str())) {
					Ok(()) => Ok(KValue::Str(dest.clone())),
					Err(error) => runtime_error!("failed to build ramdisk: {error}"),
				}
			}
			unexpected => unexpected_args("|String|", unexpected),
		}
	}
}

impl KotoObject for RamdiskBuilder {
	fn display(&self, ctx: &mut DisplayContext) -> KotoResult<()> {
		ctx.append(format!("ramdisk(items: {})", self.items.len()));
		Ok(())
	}
}

impl RamdiskBuilder {
	/// Encodes the ramdisk into `writer`.
	fn encode_to<W: Write + Seek>(&self, writer: &mut W) -> Result<(), String> {
		let mut encoder = Encoder::new(writer)
			.map_err(|error| format!("failed to write ramdisk header: {error:?}"))?;

		for item in &self.items {
			match item {
				ItemSpec::Kernel {
					file,
					arch,
					compression,
				} => {
					let data = open(file)?;
					let result = match compression {
						None => encoder.write_item(KernelEncoder::new_uncompressed(data, *arch)),
						Some(level) => {
							encoder.write_item(KernelEncoder::new_compress_with_zstd(
								data, *level, *arch,
							))
						}
					};
					result.map_err(|error| {
						format!("failed to encode {arch:?} kernel item: {error:?}")
					})?;
				}
			}
		}

		encoder
			.finish()
			.map_err(|error| format!("failed to finalize ramdisk: {error:?}"))?;

		Ok(())
	}

	/// Returns the ramdisk as a [`File`] whose contents are encoded on
	/// demand, so that a ramdisk over not-yet-built artifacts can be
	/// placed into an image's filesystem tree.
	pub fn to_file(&self) -> File {
		File::generated(self.clone())
	}

	/// Encodes the ramdisk and writes it to `dest`, creating the
	/// destination's parent directories as needed.
	fn build_to(&self, dest: &std::path::Path) -> Result<(), String> {
		if let Some(parent) = dest.parent().filter(|p| !p.as_os_str().is_empty()) {
			std::fs::create_dir_all(parent)
				.map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
		}

		let mut out = std::fs::File::create(dest)
			.map_err(|error| format!("failed to create {}: {error}", dest.display()))?;

		self.encode_to(&mut out)
	}

	/// Encodes the ramdisk into a new byte buffer.
	pub fn encode(&self) -> Result<Vec<u8>, String> {
		let mut out = Vec::new();
		self.encode_to(&mut std::io::Cursor::new(&mut out))?;
		Ok(out)
	}
}

impl GenerateFile for RamdiskBuilder {
	fn generate(&self) -> Result<Vec<u8>, String> {
		self.encode()
	}
}

/// Opens a [`File`]'s contents as a reader.
fn open(file: &File) -> Result<Box<dyn Read>, String> {
	match &file.source {
		FileSource::Disk(path) => {
			if !path.is_file() {
				return Err(format!(
					"missing file {} (artifact binaries must be built first, e.g. via `cargo oro \
					 build`)",
					path.display()
				));
			}

			std::fs::File::open(path)
				.map(|file| Box::new(file) as Box<dyn Read>)
				.map_err(|error| format!("failed to read {}: {error}", path.display()))
		}
		FileSource::Memory(contents) => Ok(Box::new(std::io::Cursor::new(contents.clone()))),
		FileSource::Generated(generator) => {
			Ok(Box::new(std::io::Cursor::new(generator.generate()?)))
		}
	}
}

/// Creates the `oro.ramdisk` module.
pub fn make_module() -> KMap {
	let map = KMap::with_type("ramdisk");

	// `ramdisk.builder()` begins a new ramdisk specification.
	map.add_fn("builder", |ctx| {
		match ctx.args() {
			[] => Ok(KValue::Object(KObject::from(RamdiskBuilder::default()))),
			unexpected => unexpected_args("||", unexpected),
		}
	});

	// `ramdisk.kernel <file>, <arch>` specifies a kernel item.
	map.add_fn("kernel", |ctx| {
		match ctx.args() {
			[KValue::Object(file), KValue::Str(arch)] => {
				let file = file.cast::<File>()?.clone();
				let arch = match parse_arch(arch.as_str()) {
					Ok(arch) => arch,
					Err(error) => return runtime_error!("ramdisk.kernel: {error}"),
				};

				Ok(KValue::Object(KObject::from(Item(ItemSpec::Kernel {
					file,
					arch,
					compression: None,
				}))))
			}
			unexpected => unexpected_args("|File, String|", unexpected),
		}
	});

	map
}

/// Parses an architecture name as written in the packaging scripts,
/// matching the spellings accepted by `package.arch`.
fn parse_arch(name: &str) -> Result<KernelArch, String> {
	match name {
		"x86_64" => Ok(KernelArch::X86_64),
		"aarch64" => Ok(KernelArch::Aarch64),
		"riscv64" => Ok(KernelArch::Riscv64),
		other => {
			Err(format!(
				"unknown architecture '{other}' (expected 'x86_64', 'aarch64' or 'riscv64')"
			))
		}
	}
}

#[cfg(test)]
mod tests {
	use koto::Koto;
	use oro_ramdisk::{Decoder, item::Kernel};

	use super::*;

	/// Runs `script` against a prelude holding just the `oro.ramdisk`
	/// and `oro.file` modules, returning the script's result.
	fn run_script(script: &str) -> std::result::Result<KValue, String> {
		let mut koto = Koto::default();
		let oro = KMap::with_type("oro");
		oro.insert("ramdisk", make_module());
		oro.insert("file", crate::oro::file::make_module());
		koto.prelude().insert("oro", oro);
		koto.compile_and_run(script)
			.map_err(|error| error.to_string())
	}

	/// Decodes a `File` produced by `ramdisk.file()`.
	fn decode_generated(value: KValue) -> Vec<u8> {
		let KValue::Object(object) = value else {
			panic!("ramdisk.file() should return a file object");
		};
		let file = object.cast::<File>().expect("the object should be a File");
		let FileSource::Generated(generator) = &file.source else {
			panic!("ramdisk.file() should produce a generated file");
		};
		generator.generate().expect("generation should succeed")
	}

	#[test]
	fn encodes_uncompressed_kernel_item() {
		let builder = RamdiskBuilder {
			items: vec![ItemSpec::Kernel {
				file: File::memory(b"fake kernel image".to_vec()),
				arch: KernelArch::X86_64,
				compression: None,
			}],
		};

		let encoded = builder.encode().expect("encode should succeed");

		let decoder = Decoder::new(&encoded).expect("output should be a valid ramdisk");
		assert_eq!(decoder.index_items().len(), 1);

		let kernel = decoder
			.item::<Kernel>(0)
			.expect("item 0 should be a kernel");
		assert!(!kernel.flags.compressed);
		assert_eq!(kernel.flags.arch, KernelArch::X86_64);
		assert_eq!(kernel.data, b"fake kernel image");
	}

	#[test]
	fn encodes_compressed_kernel_item() {
		let builder = RamdiskBuilder {
			items: vec![ItemSpec::Kernel {
				file: File::memory(b"fake kernel image".to_vec()),
				arch: KernelArch::Aarch64,
				compression: Some(3),
			}],
		};

		let encoded = builder.encode().expect("encode should succeed");

		let decoder = Decoder::new(&encoded).expect("output should be a valid ramdisk");
		let kernel = decoder
			.item::<Kernel>(0)
			.expect("item 0 should be a kernel");
		assert!(kernel.flags.compressed);
		assert_eq!(kernel.flags.arch, KernelArch::Aarch64);

		let mut stream = kernel
			.decompression_stream()
			.expect("compressed kernels expose a decompression stream")
			.expect("the stream should be a valid zstd frame");
		let mut decompressed = Vec::new();
		stream
			.read_to_end(&mut decompressed)
			.expect("decompression should succeed");
		assert_eq!(decompressed, b"fake kernel image");
	}

	#[test]
	fn parses_the_architecture_names_used_by_packaging_scripts() {
		assert_eq!(parse_arch("x86_64"), Ok(KernelArch::X86_64));
		assert_eq!(parse_arch("aarch64"), Ok(KernelArch::Aarch64));
		assert_eq!(parse_arch("riscv64"), Ok(KernelArch::Riscv64));
	}

	#[test]
	fn rejects_an_unknown_architecture_name_by_listing_the_valid_ones() {
		let error = parse_arch("x86-64").expect_err("'x86-64' is not a valid architecture");
		assert_eq!(
			error,
			"unknown architecture 'x86-64' (expected 'x86_64', 'aarch64' or 'riscv64')"
		);
	}

	#[test]
	fn reports_a_missing_artifact_binary_with_a_build_hint() {
		let missing = std::env::temp_dir().join("oro-ramdisk-test-does-not-exist");
		let builder = RamdiskBuilder {
			items: vec![ItemSpec::Kernel {
				file: File::disk(&missing),
				arch: KernelArch::X86_64,
				compression: None,
			}],
		};

		let error = builder
			.encode()
			.expect_err("a missing image cannot be encoded");

		assert_eq!(
			error,
			format!(
				"missing file {} (artifact binaries must be built first, e.g. via `cargo oro \
				 build`)",
				missing.display()
			)
		);
	}

	#[test]
	fn exposes_itself_as_a_generated_file_holding_the_encoded_ramdisk() {
		let builder = RamdiskBuilder {
			items: vec![ItemSpec::Kernel {
				file: File::memory(b"fake kernel image".to_vec()),
				arch: KernelArch::Riscv64,
				compression: None,
			}],
		};

		let file = builder.to_file();
		let FileSource::Generated(generator) = &file.source else {
			panic!("ramdisk.file() should produce a generated file");
		};

		let bytes = generator.generate().expect("generation should succeed");
		let decoder = Decoder::new(&bytes).expect("output should be a valid ramdisk");
		let kernel = decoder
			.item::<Kernel>(0)
			.expect("item 0 should be a kernel");
		assert_eq!(kernel.flags.arch, KernelArch::Riscv64);
		assert_eq!(kernel.data, b"fake kernel image");
	}

	#[test]
	fn reads_its_inputs_when_generated_rather_than_when_specified() {
		let image = std::env::temp_dir().join("oro-ramdisk-test-deferred-read");
		std::fs::write(&image, b"stale").expect("the test image should be writable");

		let file = RamdiskBuilder {
			items: vec![ItemSpec::Kernel {
				file: File::disk(&image),
				arch: KernelArch::X86_64,
				compression: None,
			}],
		}
		.to_file();

		// Rewritten *after* the spec was built: a generated file must
		// see this, not what was on disk when `.file()` was called.
		std::fs::write(&image, b"freshly built").expect("the test image should be writable");

		let FileSource::Generated(generator) = &file.source else {
			panic!("ramdisk.file() should produce a generated file");
		};
		let bytes = generator.generate().expect("generation should succeed");
		std::fs::remove_file(&image).expect("the test image should be removable");

		let decoder = Decoder::new(&bytes).expect("output should be a valid ramdisk");
		let kernel = decoder
			.item::<Kernel>(0)
			.expect("item 0 should be a kernel");
		assert_eq!(kernel.data, b"freshly built");
	}

	#[test]
	fn writes_a_standalone_ramdisk_to_the_destination_path() {
		let dest = std::env::temp_dir()
			.join("oro-ramdisk-test-build")
			.join("nested")
			.join("out.rd");
		let _ = std::fs::remove_dir_all(dest.parent().unwrap());

		RamdiskBuilder {
			items: vec![ItemSpec::Kernel {
				file: File::memory(b"fake kernel image".to_vec()),
				arch: KernelArch::X86_64,
				compression: None,
			}],
		}
		.build_to(&dest)
		.expect("build should succeed");

		let written = std::fs::read(&dest).expect("the ramdisk should have been written");
		let _ = std::fs::remove_dir_all(dest.parent().unwrap());

		let decoder = Decoder::new(&written).expect("output should be a valid ramdisk");
		let kernel = decoder
			.item::<Kernel>(0)
			.expect("item 0 should be a kernel");
		assert_eq!(kernel.data, b"fake kernel image");
	}

	#[test]
	fn builds_a_ramdisk_from_a_koto_script() {
		let result = run_script(
			"
from oro import ramdisk, file
builder = ramdisk.builder()
builder.item ramdisk.kernel(file.from_str('fake kernel image'), 'riscv64')
builder.file()
",
		)
		.expect("the script should run");

		let encoded = decode_generated(result);
		let decoder = Decoder::new(&encoded).expect("output should be a valid ramdisk");
		assert_eq!(decoder.index_items().len(), 1);

		let kernel = decoder
			.item::<Kernel>(0)
			.expect("item 0 should be a kernel");
		assert!(!kernel.flags.compressed);
		assert_eq!(kernel.flags.arch, KernelArch::Riscv64);
		assert_eq!(kernel.data, b"fake kernel image");
	}

	#[test]
	fn compresses_a_kernel_when_the_script_asks_for_it() {
		let result = run_script(
			"
from oro import ramdisk, file
builder = ramdisk.builder()
builder.item ramdisk.kernel(file.from_str('fake kernel image'), 'x86_64').compress()
builder.file()
",
		)
		.expect("the script should run");

		let encoded = decode_generated(result);
		let decoder = Decoder::new(&encoded).expect("output should be a valid ramdisk");
		let kernel = decoder
			.item::<Kernel>(0)
			.expect("item 0 should be a kernel");
		assert!(kernel.flags.compressed);
	}

	#[test]
	fn rejects_an_unknown_architecture_from_a_koto_script() {
		let error = run_script(
			"
from oro import ramdisk, file
ramdisk.kernel file.from_str('fake kernel image'), 'x86-64'
",
		)
		.expect_err("'x86-64' is not a valid architecture");

		assert!(
			error.contains("unknown architecture 'x86-64'"),
			"unexpected error: {error}"
		);
	}

	#[test]
	fn rejects_a_compression_level_outside_the_zstd_range() {
		let error = run_script(
			"
from oro import ramdisk, file
ramdisk.kernel(file.from_str('fake kernel image'), 'x86_64').compress 99
",
		)
		.expect_err("99 is outside zstd's level range");

		assert!(
			error.contains("compression level"),
			"unexpected error: {error}"
		);
	}

	#[test]
	fn writes_a_ramdisk_from_a_koto_script() {
		let dest = std::env::temp_dir().join("oro-ramdisk-test-script-build.rd");
		let _ = std::fs::remove_file(&dest);

		let result = run_script(&format!(
			"
from oro import ramdisk, file
builder = ramdisk.builder()
builder.item ramdisk.kernel(file.from_str('fake kernel image'), 'aarch64')
builder.build '{}'
",
			dest.display()
		))
		.expect("the script should run");

		let KValue::Str(returned) = result else {
			panic!("ramdisk.build should return the destination path");
		};
		assert_eq!(returned.as_str(), dest.to_string_lossy());

		let written = std::fs::read(&dest).expect("the ramdisk should have been written");
		let _ = std::fs::remove_file(&dest);

		let decoder = Decoder::new(&written).expect("output should be a valid ramdisk");
		let kernel = decoder
			.item::<Kernel>(0)
			.expect("item 0 should be a kernel");
		assert_eq!(kernel.flags.arch, KernelArch::Aarch64);
	}
}
