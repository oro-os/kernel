//! The `oro.limine` module; Limine-specific packaging actions.

use std::path::Path;

use koto::prelude::*;

use super::file::{File, FileSource};

/// Creates the `oro.limine` module.
pub fn make_module() -> KMap {
	let map = KMap::with_type("limine");

	// `limine.bios_install <image path>, <blob>` installs the Limine
	// legacy-BIOS stages onto the image, where `<blob>` is a file
	// reference to the raw `limine-bios-hdd` stage blob.
	map.add_fn("bios_install", |ctx| {
		match ctx.args() {
			[KValue::Str(image), KValue::Object(blob)] => {
				let stage_blob = blob.cast::<File>()?.clone();
				let blob = match &stage_blob.source {
					FileSource::Disk(path) => {
						match std::fs::read(path) {
							Ok(contents) => contents,
							Err(error) => {
								return runtime_error!(
									"limine.bios_install: failed to read stage blob {}: {error}",
									path.display()
								);
							}
						}
					}
					FileSource::Memory(contents) => contents.clone(),
				};

				match crate::limine::bios_install(Path::new(image.as_str()), &blob) {
					Ok(()) => Ok(KValue::Null),
					Err(error) => runtime_error!("limine.bios_install: {error}"),
				}
			}
			unexpected => unexpected_args("|String, File|", unexpected),
		}
	});

	map
}
