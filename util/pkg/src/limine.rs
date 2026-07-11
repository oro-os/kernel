//! Rust port of Limine's `bios-install` utility (`limine.c`, BSD-2-Clause,
//! Copyright (C) 2019-2025 mintsuki and contributors).
//!
//! Installs Limine's legacy-BIOS boot stages onto a GPT disk image:
//!
//! - stage 1 (the first 512 bytes of the `limine-bios-hdd` blob) is
//!   written to LBA 0, preserving the image's disk timestamp (bytes
//!   218..224) and disk signature + partition table (bytes 440..510);
//! - stage 2 (the rest of the blob) is split into two halves, embedded
//!   into the unused tails of the primary and backup GPT partition
//!   entry arrays (shrinking them, with headers re-CRC'd); and
//! - the halves' sizes and byte locations are patched into the boot
//!   sector at offset 0x1a4 so that stage 1 can find them.
//!
//! Only the GPT embedding path of the original utility is ported; the
//! packager always produces GPT images. The MBR post-gap path, the
//! install-to-partition path, and the uninstall machinery (pointless
//! for freshly generated images) are intentionally omitted.

use std::{
	io::{Read, Seek, SeekFrom, Write},
	path::Path,
};

/// The size of the on-disk GPT header (the portion that is CRC'd).
const GPT_HEADER_SIZE: usize = 92;

/// Offsets within the GPT header.
const GPT_CRC32: usize = 16;
const GPT_ALTERNATE_LBA: usize = 32;
const GPT_PARTITION_ENTRY_LBA: usize = 72;
const GPT_NUMBER_OF_PARTITION_ENTRIES: usize = 80;
const GPT_SIZE_OF_PARTITION_ENTRY: usize = 84;
const GPT_PARTITION_ENTRY_ARRAY_CRC32: usize = 88;

/// The number of LBAs nominally reserved for a 128-entry GPT partition
/// entry array; the stage 2 halves are placed at the end of this region.
const GPT_PARTITION_ARRAY_LBAS: u64 = 32;

/// The offset within the boot sector where the stage 2 sizes and
/// locations are patched in (two u16 sizes followed by two u64
/// locations, little-endian; ends exactly at the partition table area
/// at byte 440).
const STAGE2_PATCH_OFFSET: u64 = 0x1A4;

fn u32_at(raw: &[u8], offset: usize) -> u32 {
	u32::from_le_bytes(raw[offset..offset + 4].try_into().unwrap())
}

fn u64_at(raw: &[u8], offset: usize) -> u64 {
	u64::from_le_bytes(raw[offset..offset + 8].try_into().unwrap())
}

fn set_u32_at(raw: &mut [u8], offset: usize, value: u32) {
	raw[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

/// Installs the Limine BIOS stages from the given `limine-bios-hdd`
/// blob onto the GPT image at `image_path`.
pub fn bios_install(image_path: &Path, bootloader: &[u8]) -> Result<(), String> {
	if bootloader.len() <= 512 {
		return Err(format!(
			"limine bios stage blob is too small ({} bytes); expected a boot sector followed by \
			 stage 2",
			bootloader.len()
		));
	}

	let mut device = std::fs::OpenOptions::new()
		.read(true)
		.write(true)
		.open(image_path)
		.map_err(|e| format!("failed to open {}: {e}", image_path.display()))?;

	let read_at = |device: &mut std::fs::File, location: u64, buffer: &mut [u8]| {
		device
			.seek(SeekFrom::Start(location))
			.and_then(|_| device.read_exact(buffer))
			.map_err(|e| format!("failed to read {} at {location}: {e}", image_path.display()))
	};
	let write_at = |device: &mut std::fs::File, location: u64, buffer: &[u8]| {
		device
			.seek(SeekFrom::Start(location))
			.and_then(|_| device.write_all(buffer))
			.map_err(|e| {
				format!(
					"failed to write {} at {location}: {e}",
					image_path.display()
				)
			})
	};

	// Probe for a GPT, guessing the logical block size.
	let mut gpt = None;
	for lb_size in [512u64, 4096] {
		let mut header = [0u8; GPT_HEADER_SIZE];
		read_at(&mut device, lb_size, &mut header)?;
		if &header[0..8] == b"EFI PART" {
			gpt = Some((lb_size, header));
			break;
		}
	}
	let Some((lb_size, mut primary)) = gpt else {
		return Err(format!(
			"{} has no GPT; only GPT images are supported",
			image_path.display()
		));
	};

	let alternate_lba = u64_at(&primary, GPT_ALTERNATE_LBA);
	let mut secondary = [0u8; GPT_HEADER_SIZE];
	read_at(&mut device, alternate_lba * lb_size, &mut secondary)?;
	if &secondary[0..8] != b"EFI PART" {
		return Err(format!(
			"{}: secondary GPT header is not valid",
			image_path.display()
		));
	}

	// Split stage 2 into two halves, sized in whole 512-byte sectors
	// (the first half gets the odd sector).
	let stage2_size = bootloader.len() as u64 - 512;
	let stage2_sects = stage2_size.div_ceil(512);
	let stage2_size_a = (stage2_sects / 2) * 512 + (stage2_sects % 2) * 512;
	let stage2_size_b = (stage2_sects / 2) * 512;

	let primary_entry_lba = u64_at(&primary, GPT_PARTITION_ENTRY_LBA);
	let secondary_entry_lba = u64_at(&secondary, GPT_PARTITION_ENTRY_LBA);
	let entry_size = u64::from(u32_at(&primary, GPT_SIZE_OF_PARTITION_ENTRY));
	let entry_count = u64::from(u32_at(&primary, GPT_NUMBER_OF_PARTITION_ENTRIES));

	// Place each half at the end of a partition entry array's reserved
	// region, aligned down to the logical block size.
	let stage2_loc_a =
		((primary_entry_lba + GPT_PARTITION_ARRAY_LBAS) * lb_size - stage2_size_a) & !(lb_size - 1);
	let stage2_loc_b = ((secondary_entry_lba + GPT_PARTITION_ARRAY_LBAS) * lb_size - stage2_size_b)
		& !(lb_size - 1);

	// Shrinking the arrays must not clobber any used partition entry.
	let mut max_used_entry: Option<u64> = None;
	for i in 0..entry_count {
		// The unique partition GUID (offset 16) is non-zero for used entries.
		let mut guid = [0u8; 16];
		read_at(
			&mut device,
			primary_entry_lba * lb_size + i * entry_size + 16,
			&mut guid,
		)?;
		if guid != [0u8; 16] {
			max_used_entry = Some(i);
		}
	}

	let entries_per_lb = lb_size / entry_size;
	let new_array_lba_size = stage2_loc_a / lb_size - primary_entry_lba;
	let new_entry_count = new_array_lba_size * entries_per_lb;

	if max_used_entry.is_some_and(|max| new_entry_count <= max) {
		return Err(format!(
			"{}: cannot embed stage 2: too many used GPT partition entries",
			image_path.display()
		));
	}

	// Zero the entries freed by the shrink in both arrays.
	let zero_entry = vec![0u8; entry_size as usize];
	let first_free_entry = max_used_entry.map_or(0, |max| max + 1);
	for i in first_free_entry..new_entry_count {
		write_at(
			&mut device,
			primary_entry_lba * lb_size + i * entry_size,
			&zero_entry,
		)?;
		write_at(
			&mut device,
			secondary_entry_lba * lb_size + i * entry_size,
			&zero_entry,
		)?;
	}

	// Recompute the entry array CRC over the shrunk primary array (the
	// backup array is identical) and re-CRC both headers.
	let mut array = vec![0u8; (new_entry_count * entry_size) as usize];
	read_at(&mut device, primary_entry_lba * lb_size, &mut array)?;
	let array_crc32 = crc32fast::hash(&array);

	for (header, location) in [
		(&mut primary, lb_size),
		(&mut secondary, alternate_lba * lb_size),
	] {
		set_u32_at(header, GPT_PARTITION_ENTRY_ARRAY_CRC32, array_crc32);
		set_u32_at(
			header,
			GPT_NUMBER_OF_PARTITION_ENTRIES,
			new_entry_count as u32,
		);
		set_u32_at(header, GPT_CRC32, 0);
		let crc = crc32fast::hash(header);
		set_u32_at(header, GPT_CRC32, crc);
		write_at(&mut device, location, header)?;
	}

	// Save the parts of LBA 0 that must survive the boot sector write.
	let mut timestamp = [0u8; 6];
	read_at(&mut device, 218, &mut timestamp)?;
	let mut partition_table = [0u8; 70];
	read_at(&mut device, 440, &mut partition_table)?;

	// Write the boot sector and the stage 2 halves.
	write_at(&mut device, 0, &bootloader[..512])?;
	let half_a_len = stage2_size_a.min(stage2_size) as usize;
	write_at(
		&mut device,
		stage2_loc_a,
		&bootloader[512..512 + half_a_len],
	)?;
	write_at(&mut device, stage2_loc_b, &bootloader[512 + half_a_len..])?;

	// Patch the halves' sizes and locations into the boot sector.
	let mut patch = [0u8; 20];
	patch[0..2].copy_from_slice(&(stage2_size_a as u16).to_le_bytes());
	patch[2..4].copy_from_slice(&(stage2_size_b as u16).to_le_bytes());
	patch[4..12].copy_from_slice(&stage2_loc_a.to_le_bytes());
	patch[12..20].copy_from_slice(&stage2_loc_b.to_le_bytes());
	write_at(&mut device, STAGE2_PATCH_OFFSET, &patch)?;

	// Restore the saved regions.
	write_at(&mut device, 218, &timestamp)?;
	write_at(&mut device, 440, &partition_table)?;

	device
		.flush()
		.map_err(|e| format!("failed to flush {}: {e}", image_path.display()))?;

	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;

	/// Builds a minimal-but-valid GPT image: protective MBR-ish LBA 0,
	/// primary header at LBA 1 with its entry array at LBA 2, and a
	/// backup header at the last LBA with its entry array before it.
	fn make_gpt_image(total_lbas: u64, used_entries: u64) -> Vec<u8> {
		const LB: u64 = 512;
		let mut image = vec![0u8; (total_lbas * LB) as usize];

		let alternate_lba = total_lbas - 1;
		let primary_entry_lba = 2u64;
		let secondary_entry_lba = alternate_lba - GPT_PARTITION_ARRAY_LBAS;

		// Fake partition table entries + boot signature in LBA 0.
		image[440..510].fill(0xAA);
		image[218..224].fill(0xBB);

		let make_header = |my_lba: u64, alt_lba: u64, entry_lba: u64| {
			let mut header = [0u8; GPT_HEADER_SIZE];
			header[0..8].copy_from_slice(b"EFI PART");
			header[24..32].copy_from_slice(&my_lba.to_le_bytes());
			header[GPT_ALTERNATE_LBA..GPT_ALTERNATE_LBA + 8]
				.copy_from_slice(&alt_lba.to_le_bytes());
			header[GPT_PARTITION_ENTRY_LBA..GPT_PARTITION_ENTRY_LBA + 8]
				.copy_from_slice(&entry_lba.to_le_bytes());
			header[GPT_NUMBER_OF_PARTITION_ENTRIES..GPT_NUMBER_OF_PARTITION_ENTRIES + 4]
				.copy_from_slice(&128u32.to_le_bytes());
			header[GPT_SIZE_OF_PARTITION_ENTRY..GPT_SIZE_OF_PARTITION_ENTRY + 4]
				.copy_from_slice(&128u32.to_le_bytes());
			header
		};

		let primary = make_header(1, alternate_lba, primary_entry_lba);
		let secondary = make_header(alternate_lba, 1, secondary_entry_lba);
		image[(LB as usize)..(LB as usize) + GPT_HEADER_SIZE].copy_from_slice(&primary);
		let secondary_offset = (alternate_lba * LB) as usize;
		image[secondary_offset..secondary_offset + GPT_HEADER_SIZE].copy_from_slice(&secondary);

		// Mark the first `used_entries` partition entries as used in
		// both arrays.
		for i in 0..used_entries {
			for entry_lba in [primary_entry_lba, secondary_entry_lba] {
				let offset = (entry_lba * LB + i * 128 + 16) as usize;
				image[offset..offset + 16].fill(0xCC);
			}
		}

		image
	}

	#[test]
	fn installs_into_gpt_image() {
		let dir = std::env::temp_dir().join("orok-util-pkg-test-limine");
		std::fs::create_dir_all(&dir).unwrap();
		let image_path = dir.join("test.img");

		let image = make_gpt_image(128, 2);
		std::fs::write(&image_path, &image).unwrap();

		// A fake blob: one stage 1 sector plus 5 sectors of stage 2.
		let mut blob = vec![0u8; 512 * 6];
		blob[..512].fill(0xEE);
		blob[512..].fill(0xDD);
		bios_install(&image_path, &blob).unwrap();

		let patched = std::fs::read(&image_path).unwrap();

		// Stage 1 was written, with the timestamp and partition table
		// areas preserved.
		assert!(patched[0..218].iter().all(|b| *b == 0xEE));
		assert!(patched[218..224].iter().all(|b| *b == 0xBB));
		assert!(patched[224..420].iter().all(|b| *b == 0xEE));
		assert!(patched[440..510].iter().all(|b| *b == 0xAA));

		// The patched sizes and locations point at stage 2 data.
		let size_a = u16::from_le_bytes(patched[0x1A4..0x1A6].try_into().unwrap()) as u64;
		let size_b = u16::from_le_bytes(patched[0x1A6..0x1A8].try_into().unwrap()) as u64;
		let loc_a = u64_at(&patched, 0x1A8);
		let loc_b = u64_at(&patched, 0x1B0);
		assert_eq!(size_a + size_b, 512 * 5);
		assert!(
			patched[loc_a as usize..(loc_a + size_a) as usize]
				.iter()
				.all(|b| *b == 0xDD)
		);
		assert!(
			patched[loc_b as usize..(loc_b + size_b) as usize]
				.iter()
				.all(|b| *b == 0xDD)
		);

		// Both GPT headers are self-consistent: entry counts shrunk,
		// header CRCs valid, and array CRCs matching the arrays.
		for header_lba in [1u64, 127] {
			let offset = (header_lba * 512) as usize;
			let mut header: [u8; GPT_HEADER_SIZE] = patched[offset..offset + GPT_HEADER_SIZE]
				.try_into()
				.unwrap();

			let new_count = u64::from(u32_at(&header, GPT_NUMBER_OF_PARTITION_ENTRIES));
			assert!(new_count < 128);
			assert!(new_count >= 2);

			let expected_crc = u32_at(&header, GPT_CRC32);
			set_u32_at(&mut header, GPT_CRC32, 0);
			assert_eq!(crc32fast::hash(&header), expected_crc);

			let entry_lba = u64_at(&header, GPT_PARTITION_ENTRY_LBA);
			let array_offset = (entry_lba * 512) as usize;
			let array = &patched[array_offset..array_offset + (new_count * 128) as usize];
			assert_eq!(
				crc32fast::hash(array),
				u32_at(&header, GPT_PARTITION_ENTRY_ARRAY_CRC32)
			);
		}
	}
}
