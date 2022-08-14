use std::env;
use std::fs;
use std::path::Path;

use graphicsmagick::wand::{DrawingWand, MagickWand, PixelWand};

const FONT_FILE: &str = "asset/EnterCommand.ttf";
const FONT_POINT_SIZE: usize = 16;
const FONT_HEIGHT: usize = 10;
const FONT_WIDTH: usize = 6;
const FONT_BASELINE: usize = 8;

// NOTE: The first character (index 0) is used as the 'unknown character'
// NOTE: glyph and is inverted whenever it is shown. Make sure it's a glyph
// NOTE: that makes sense.
const FONT_CHARSET: &str =
	"?ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*()`~[]{}=+'\"\\|/.,<> -_:;";

fn main() {
	graphicsmagick::initialize();

	let src_dir = env::var_os("CARGO_MANIFEST_DIR").unwrap();

	if FONT_CHARSET.len() > 255 {
		// Not 256, because 255 (0xFF) is reserved to mean "invalid".
		panic!("charset has too many characters; maximum count 255");
	}

	let mut charset_lookup = [255u8; 256];
	for (i, c) in FONT_CHARSET.bytes().enumerate() {
		if charset_lookup[c as usize] != 255 {
			panic!("duplicate charset character at index {}", i);
		}

		charset_lookup[c as usize] = i as u8;
	}

	let font_path = Path::new(&src_dir).join(FONT_FILE);

	let font = DrawingWand::new()
		.set_font(font_path.to_str().unwrap())
		.set_font_size(FONT_POINT_SIZE as f64)
		.set_fill_color(PixelWand::new().set_color("#FFFFFF"))
		.set_text_antialias(0)
		.clone();

	let mut font_img = MagickWand::new()
		.set_size((FONT_WIDTH * FONT_CHARSET.len()) as u64, FONT_HEIGHT as u64)
		.unwrap()
		.read_image("xc:black")
		.unwrap()
		.set_image_depth(1)
		.unwrap()
		.clone();

	FONT_CHARSET
		.chars()
		.into_iter()
		.enumerate()
		.for_each(|(i, c)| {
			font_img
				.annotate_image(
					&font,
					(i * FONT_WIDTH) as f64,
					FONT_BASELINE as f64,
					0.0,
					&c.to_string(),
				)
				.unwrap();
		});

	let font_blob = font_img
		.clone()
		.set_image_format("GRAY")
		.unwrap()
		.write_image_blob()
		.unwrap()
		.chunks(8)
		.map(|c| {
			c.iter().fold(0u8, |acc, el| {
				(acc << 1)
					| match el {
						0 => 0,
						_ => 1,
					}
			})
		})
		.collect::<Vec<u8>>();

	let out_dir = env::var_os("OUT_DIR").unwrap();

	{
		let dest_path = Path::new(&out_dir).join("font.bin");
		fs::write(&dest_path, font_blob).unwrap();
	}

	{
		let dest_path = Path::new(&out_dir).join("font.png");
		fs::write(
			&dest_path,
			font_img
				.set_image_format("PNG")
				.unwrap()
				.write_image_blob()
				.unwrap(),
		)
		.unwrap();
	}

	{
		let dest_path = Path::new(&out_dir).join("oro_font.rs");
		fs::write(
			&dest_path,
			format!(
				"
					/// Raw font data packed in from the TTF font build step.
					const FONT_BITS: &[u8] = core::include_bytes!(\"font.bin\");
					/// The number of horizontal bits per glyph
					const FONT_GLYPH_WIDTH: usize = {};
					/// The number of vertical bits per glyph
					const FONT_GLYPH_HEIGHT: usize = {};
					/// The number of bits per font atlas scanline
					const FONT_GLYPH_STRIDE_BITS: usize = {};
					/// ASCII -> glyph lookup table
					const FONT_GLYPH_LOOKUP: [u8;256] = [{}];
				",
				FONT_WIDTH,
				FONT_HEIGHT,
				FONT_WIDTH * FONT_CHARSET.len(),
				charset_lookup
					.iter()
					.map(|&c| c.to_string())
					.collect::<Vec<String>>()
					.join(",")
			),
		)
		.unwrap();
	}

	println!("cargo:rerun-if-changed=build.rs");
	println!("cargo:rerun-if-changed={}", FONT_FILE);
}
