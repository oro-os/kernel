//! The `oro.iso` module; declarative ISO 9660 image specifications,
//! backed by [`isobemak`] at build time.

use std::path::Path;

use isobemak::iso::{
	boot_info::{BiosBootInfo, BootInfo, UefiBootInfo},
	builder::build_iso,
	disk_layout::UefiBootStrategy,
	iso_image::{IsoImage, IsoImageFile},
	layout_profile::{ElToritoMode, EspMode, HiddenSectorMode, IsoLayoutProfile, MbrMode},
};
use koto::{derive::*, prelude::*, runtime::Result};

use super::file::{File, FileSource};

/// A single entry in the ISO's filesystem tree.
#[derive(Clone, Debug)]
pub struct IsoEntry {
	/// The destination path within the ISO, `/`-separated.
	pub destination: String,
	pub file: File,
}

/// A declarative ISO specification, built up via chained setters from
/// `iso.builder()` and consumed by the packager's build step.
#[derive(Clone, Default, KotoType, KotoCopy)]
pub struct IsoBuilder {
	/// The layout profile; defaults to [`IsoLayoutProfile::hardware`].
	pub profile: IsoLayoutProfile,
	pub volume_id: Option<String>,
	pub isohybrid: bool,
	/// The `/`-separated in-ISO path of the El Torito BIOS boot image.
	pub boot_bios: Option<String>,
	/// The `/`-separated in-ISO path of the UEFI boot image.
	pub boot_uefi: Option<String>,
	/// Whether to patch an El Torito boot information table into the
	/// BIOS boot image (mkisofs's `-boot-info-table`). Required by
	/// bootloaders whose CD stage locates itself through the table
	/// (e.g. Limine).
	pub boot_info_table: bool,
	pub entries: Vec<IsoEntry>,
}

#[koto_impl(runtime = koto::runtime)]
impl IsoBuilder {
	#[koto_method]
	fn use_gpt(ctx: MethodContext<Self>) -> Result<KValue> {
		match ctx.args {
			[KValue::Bool(enabled)] => {
				ctx.instance_mut()?.profile.use_gpt = *enabled;
				ctx.instance_result()
			}
			unexpected => unexpected_args("|Bool|", unexpected),
		}
	}

	#[koto_method]
	fn eltorito_mode(ctx: MethodContext<Self>) -> Result<KValue> {
		match ctx.args {
			[KValue::Str(mode)] => {
				ctx.instance_mut()?.profile.eltorito_mode = match mode.as_str() {
					"both" => ElToritoMode::Both,
					"direct_efi_only" => ElToritoMode::DirectEfiOnly,
					other => {
						return runtime_error!(
							"unknown eltorito_mode '{other}' (expected 'both' or \
							 'direct_efi_only')"
						);
					}
				};
				ctx.instance_result()
			}
			unexpected => unexpected_args("|String|", unexpected),
		}
	}

	#[koto_method]
	fn esp_mode(ctx: MethodContext<Self>) -> Result<KValue> {
		match ctx.args {
			[KValue::Str(mode)] => {
				ctx.instance_mut()?.profile.esp_mode = match mode.as_str() {
					"appended_partition" => EspMode::AppendedPartition,
					other => {
						return runtime_error!(
							"unknown esp_mode '{other}' (expected 'appended_partition')"
						);
					}
				};
				ctx.instance_result()
			}
			unexpected => unexpected_args("|String|", unexpected),
		}
	}

	#[koto_method]
	fn esp_alignment_lba_512(ctx: MethodContext<Self>) -> Result<KValue> {
		match ctx.args {
			[KValue::Number(alignment)] => {
				ctx.instance_mut()?.profile.esp_alignment_lba_512 = u32::from(alignment);
				ctx.instance_result()
			}
			unexpected => unexpected_args("|Number|", unexpected),
		}
	}

	#[koto_method]
	fn mbr_mode(ctx: MethodContext<Self>) -> Result<KValue> {
		match ctx.args {
			[KValue::Str(mode)] => {
				ctx.instance_mut()?.profile.mbr_mode = match mode.as_str() {
					"hybrid_linux_esp" => MbrMode::HybridLinuxEsp,
					other => {
						return runtime_error!(
							"unknown mbr_mode '{other}' (expected 'hybrid_linux_esp')"
						);
					}
				};
				ctx.instance_result()
			}
			unexpected => unexpected_args("|String|", unexpected),
		}
	}

	#[koto_method]
	fn hidden_sectors_mode(ctx: MethodContext<Self>) -> Result<KValue> {
		match ctx.args {
			[KValue::Str(mode)] => {
				ctx.instance_mut()?.profile.hidden_sectors_mode = match mode.as_str() {
					"zero" => HiddenSectorMode::Zero,
					"partition_offset" => HiddenSectorMode::PartitionOffset,
					other => {
						return runtime_error!(
							"unknown hidden_sectors_mode '{other}' (expected 'zero' or \
							 'partition_offset')"
						);
					}
				};
				ctx.instance_result()
			}
			unexpected => unexpected_args("|String|", unexpected),
		}
	}

	#[koto_method]
	fn uefi_boot_strategy(ctx: MethodContext<Self>) -> Result<KValue> {
		match ctx.args {
			[KValue::Str(strategy)] => {
				ctx.instance_mut()?.profile.uefi_boot_strategy = match strategy.as_str() {
					"eltorito_direct_efi" => UefiBootStrategy::ElToritoDirectEfi,
					"esp_partition" => UefiBootStrategy::EspPartition,
					other => {
						return runtime_error!(
							"unknown uefi_boot_strategy '{other}' (expected 'eltorito_direct_efi' \
							 or 'esp_partition')"
						);
					}
				};
				ctx.instance_result()
			}
			unexpected => unexpected_args("|String|", unexpected),
		}
	}

	#[koto_method]
	fn isohybrid(ctx: MethodContext<Self>) -> Result<KValue> {
		match ctx.args {
			[KValue::Bool(enabled)] => {
				ctx.instance_mut()?.isohybrid = *enabled;
				ctx.instance_result()
			}
			unexpected => unexpected_args("|Bool|", unexpected),
		}
	}

	#[koto_method]
	fn volume_id(ctx: MethodContext<Self>) -> Result<KValue> {
		match ctx.args {
			[KValue::Str(id)] => {
				if id.len() > 32 {
					return runtime_error!("volume_id '{id}' is longer than 32 bytes");
				}
				ctx.instance_mut()?.volume_id = Some(id.to_string());
				ctx.instance_result()
			}
			unexpected => unexpected_args("|String|", unexpected),
		}
	}

	#[koto_method]
	fn boot_bios(ctx: MethodContext<Self>) -> Result<KValue> {
		let path = parse_iso_path(ctx.args)?;
		ctx.instance_mut()?.boot_bios = Some(path);
		ctx.instance_result()
	}

	#[koto_method]
	fn boot_uefi(ctx: MethodContext<Self>) -> Result<KValue> {
		let path = parse_iso_path(ctx.args)?;
		ctx.instance_mut()?.boot_uefi = Some(path);
		ctx.instance_result()
	}

	#[koto_method]
	fn boot_info_table(ctx: MethodContext<Self>) -> Result<KValue> {
		match ctx.args {
			[KValue::Bool(enabled)] => {
				ctx.instance_mut()?.boot_info_table = *enabled;
				ctx.instance_result()
			}
			unexpected => unexpected_args("|Bool|", unexpected),
		}
	}

	/// Sets the ISO's filesystem tree.
	///
	/// Takes a (possibly nested) map of names to `File` references;
	/// nested maps become directories.
	#[koto_method]
	fn filesystem(ctx: MethodContext<Self>) -> Result<KValue> {
		match ctx.args {
			[KValue::Map(tree)] => {
				let mut entries = Vec::new();
				flatten_filesystem(tree, "", &mut entries)?;
				ctx.instance_mut()?.entries = entries;
				ctx.instance_result()
			}
			unexpected => unexpected_args("|Map|", unexpected),
		}
	}

	/// Builds the ISO image at the given destination path, returning
	/// the path.
	#[koto_method]
	fn build(ctx: MethodContext<Self>) -> Result<KValue> {
		match ctx.args {
			[KValue::Str(dest)] => {
				let spec = ctx.instance()?.clone();
				match spec.build_to(Path::new(dest.as_str())) {
					Ok(()) => Ok(KValue::Str(dest.clone())),
					Err(error) => runtime_error!("failed to build iso: {error}"),
				}
			}
			unexpected => unexpected_args("|String|", unexpected),
		}
	}
}

impl IsoBuilder {
	/// Resolves the specification and writes the ISO image to `dest`
	/// via isobemak, staging in-memory files as needed.
	fn build_to(&self, dest: &Path) -> std::result::Result<(), String> {
		let Some(dest_name) = dest.file_name() else {
			return Err(format!("invalid destination path: {}", dest.display()));
		};
		let dest_dir = dest.parent().unwrap_or_else(|| Path::new("."));
		std::fs::create_dir_all(dest_dir)
			.map_err(|e| format!("failed to create {}: {e}", dest_dir.display()))?;
		let staging_dir = dest_dir.join(".staging").join(dest_name);

		// Resolve every filesystem entry to a file on disk, staging
		// in-memory files as needed.
		let mut files = Vec::with_capacity(self.entries.len());
		for entry in &self.entries {
			let source = match &entry.file.source {
				FileSource::Disk(path) => {
					if !path.is_file() {
						return Err(format!(
							"missing file for '{}': {} (artifact binaries must be built first, \
							 e.g. via `cargo oro build`)",
							entry.destination,
							path.display()
						));
					}
					path.clone()
				}
				FileSource::Memory(contents) => {
					let staged = staging_dir.join(&entry.destination);
					if let Some(parent) = staged.parent() {
						std::fs::create_dir_all(parent).map_err(|e| {
							format!(
								"failed to create staging directory {}: {e}",
								parent.display()
							)
						})?;
					}
					std::fs::write(&staged, contents)
						.map_err(|e| format!("failed to stage {}: {e}", staged.display()))?;
					staged
				}
			};

			files.push(IsoImageFile {
				source,
				destination: entry.destination.clone(),
			});
		}

		let find_boot_image = |in_iso_path: &str| {
			files
				.iter()
				.find(|f| f.destination == in_iso_path)
				.map(|f| f.source.clone())
				.ok_or_else(|| {
					format!("boot image '{in_iso_path}' is not present in the filesystem tree")
				})
		};

		let bios_boot = self
			.boot_bios
			.as_ref()
			.map(|path| {
				Ok::<_, String>(BiosBootInfo {
					boot_image: find_boot_image(path)?,
					destination_in_iso: path.clone(),
				})
			})
			.transpose()?;

		let uefi_boot = self
			.boot_uefi
			.as_ref()
			.map(|path| {
				let boot_image = find_boot_image(path)?;

				// The generated ESP mirrors the ISO tree's `EFI/BOOT/`
				// directory, so that the firmware of every packaged
				// architecture finds its `BOOT<ARCH>.EFI` (the UEFI
				// removable-media convention) — this is what makes
				// multi-architecture ISOs possible. isobemak itself adds
				// the primary boot image under the fixed name
				// `BOOTX64.EFI`, so that exact entry is skipped here; a
				// non-x86 primary (e.g. `BOOTAA64.EFI` on an
				// aarch64-only ISO) is still mirrored under its real
				// name, alongside a stray misnamed `BOOTX64.EFI` copy
				// that isobemak's fixed naming produces.
				let additional_efi_boot_files = files
					.iter()
					.filter_map(|f| {
						let name = f.destination.strip_prefix("EFI/BOOT/")?;
						if f.destination == *path && name == "BOOTX64.EFI" {
							return None;
						}
						Some((name.to_string(), f.source.clone()))
					})
					.collect();

				Ok::<_, String>(UefiBootInfo {
					// TODO(pkg): isobemak requires a kernel image in the
					// ESP (WasabiOS heritage, staged as `KERNEL.EFI`);
					// Oro's boot flow has no use for it, so the boot
					// image doubles as a stand-in.
					kernel_image: boot_image.clone(),
					boot_image,
					destination_in_iso: path.clone(),
					additional_efi_boot_files,
					grub_cfg_content: None,
				})
			})
			.transpose()?;

		let image = IsoImage {
			volume_id: self.volume_id.clone(),
			files,
			boot_info: BootInfo {
				bios_boot,
				uefi_boot,
			},
			layout_profile: self.profile.clone(),
		};

		build_iso(dest, &image, self.isohybrid).map_err(|e| e.to_string())?;

		if let (true, Some(boot_bios)) = (self.boot_info_table, &self.boot_bios) {
			let boot_image = image
				.files
				.iter()
				.find(|f| f.destination == *boot_bios)
				.expect("bios boot image was resolved above");
			let image_length = std::fs::metadata(&boot_image.source)
				.map_err(|e| {
					format!(
						"failed to read metadata of {}: {e}",
						boot_image.source.display()
					)
				})?
				.len();
			patch_boot_info_table(dest, u32::try_from(image_length).unwrap())?;
		}

		Ok(())
	}
}

/// Patches an El Torito boot information table into the ISO's default
/// (BIOS) boot image, equivalent to mkisofs's `-boot-info-table`.
///
/// The table lives at byte offset 8 of the boot image: the PVD's LBA,
/// the boot image's own LBA, its byte length, and a checksum of the
/// image from byte 64 onward (all little-endian u32s).
fn patch_boot_info_table(iso_path: &Path, image_length: u32) -> std::result::Result<(), String> {
	use std::io::{Read, Seek, SeekFrom, Write};

	const ISO_SECTOR: u64 = 2048;
	/// LBA of the primary volume descriptor.
	const PVD_LBA: u32 = 16;
	/// LBA of the El Torito boot record volume descriptor.
	const BRVD_LBA: u64 = 17;

	let mut iso = std::fs::OpenOptions::new()
		.read(true)
		.write(true)
		.open(iso_path)
		.map_err(|e| format!("failed to open {}: {e}", iso_path.display()))?;

	let read_at = |iso: &mut std::fs::File, location: u64, buffer: &mut [u8]| {
		iso.seek(SeekFrom::Start(location))
			.and_then(|_| iso.read_exact(buffer))
			.map_err(|e| format!("failed to read {} at {location}: {e}", iso_path.display()))
	};

	// Find the boot catalog via the boot record volume descriptor.
	let mut brvd = [0u8; 2048];
	read_at(&mut iso, BRVD_LBA * ISO_SECTOR, &mut brvd)?;
	if brvd[0] != 0 || &brvd[1..6] != b"CD001" {
		return Err(format!(
			"{}: no El Torito boot record volume descriptor",
			iso_path.display()
		));
	}
	let catalog_lba = u64::from(u32::from_le_bytes(brvd[0x47..0x4B].try_into().unwrap()));

	// The default entry follows the validation entry and holds the
	// BIOS boot image's location.
	let mut catalog = [0u8; 64];
	read_at(&mut iso, catalog_lba * ISO_SECTOR, &mut catalog)?;
	if catalog[0] != 0x01 {
		return Err(format!(
			"{}: invalid El Torito boot catalog validation entry",
			iso_path.display()
		));
	}
	let default_entry = &catalog[32..64];
	if default_entry[0] != 0x88 {
		return Err(format!(
			"{}: El Torito default entry is not bootable",
			iso_path.display()
		));
	}
	let image_lba = u32::from_le_bytes(default_entry[8..12].try_into().unwrap());

	// Checksum the image from byte 64 onward, as whole little-endian
	// u32 words (zero-padded at the tail).
	let mut image = vec![0u8; image_length as usize];
	read_at(&mut iso, u64::from(image_lba) * ISO_SECTOR, &mut image)?;
	let mut checksum = 0u32;
	for word in image[64..].chunks(4) {
		let mut bytes = [0u8; 4];
		bytes[..word.len()].copy_from_slice(word);
		checksum = checksum.wrapping_add(u32::from_le_bytes(bytes));
	}

	let mut table = [0u8; 16];
	table[0..4].copy_from_slice(&PVD_LBA.to_le_bytes());
	table[4..8].copy_from_slice(&image_lba.to_le_bytes());
	table[8..12].copy_from_slice(&image_length.to_le_bytes());
	table[12..16].copy_from_slice(&checksum.to_le_bytes());

	iso.seek(SeekFrom::Start(u64::from(image_lba) * ISO_SECTOR + 8))
		.and_then(|_| iso.write_all(&table))
		.map_err(|e| {
			format!(
				"failed to write boot info table to {}: {e}",
				iso_path.display()
			)
		})?;

	Ok(())
}

impl KotoObject for IsoBuilder {
	fn display(&self, ctx: &mut DisplayContext) -> Result<()> {
		ctx.append(format!(
			"iso(volume_id: {}, entries: {})",
			self.volume_id.as_deref().unwrap_or("<none>"),
			self.entries.len()
		));
		Ok(())
	}
}

/// Parses an in-ISO path argument: either a single string or a list of
/// path components, normalized to a `/`-separated string.
fn parse_iso_path(args: &[KValue]) -> Result<String> {
	match args {
		[KValue::Str(path)] => Ok(path.trim_matches('/').to_string()),
		[KValue::List(components)] => {
			let mut path = Vec::with_capacity(components.len());
			for component in components.data().iter() {
				match component {
					KValue::Str(component) => path.push(component.to_string()),
					_ => return unexpected_args("|String| or |List of Strings|", args),
				}
			}
			Ok(path.join("/"))
		}
		unexpected => unexpected_args("|String| or |List of Strings|", unexpected),
	}
}

fn flatten_filesystem(tree: &KMap, prefix: &str, entries: &mut Vec<IsoEntry>) -> Result<()> {
	for (key, value) in tree.data().iter() {
		let KValue::Str(name) = key.value() else {
			return runtime_error!(
				"filesystem entry names must be strings, found '{}'",
				key.value().type_as_string()
			);
		};

		let destination = if prefix.is_empty() {
			name.to_string()
		} else {
			format!("{prefix}/{name}")
		};

		match value {
			KValue::Map(subtree) => flatten_filesystem(subtree, &destination, entries)?,
			KValue::Object(object) => {
				let file = object.cast::<File>()?;
				entries.push(IsoEntry {
					destination,
					file: file.clone(),
				});
			}
			unexpected => {
				return runtime_error!(
					"filesystem entry '{destination}' must be a file or a map, found '{}'",
					unexpected.type_as_string()
				);
			}
		}
	}

	Ok(())
}

/// Creates the `oro.iso` module.
pub fn make_module() -> KMap {
	let map = KMap::with_type("iso");

	map.add_fn("builder", |ctx| {
		match ctx.args() {
			[] => Ok(KValue::Object(KObject::from(IsoBuilder::default()))),
			unexpected => unexpected_args("||", unexpected),
		}
	});

	map
}
