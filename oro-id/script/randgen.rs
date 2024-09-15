#!/usr/bin/env -S cargo -Zscript
---
[package]
edition = "2021"
[dependencies]
clap = { version = "4.2", features = ["derive"] }
oro-id = { path = ".." }
rand = "0.8"
---

use clap::Parser;
use oro_id::Id;
use rand::prelude::*;

/// Generate a random Oro ID.
#[derive(Parser, Debug)]
#[clap(version)]
struct Args {
	/// Generate module IDs.
	#[clap(short='m', long="module")]
	module: bool,
	/// Generate port type IDs.
	#[clap(short='p', long="port-type")]
	port_type: bool,
	/// Make the ID internal (bits [60:32] cleared).
	/// Can only be used if developing kernel modules.
	#[clap(short='i', long="internal")]
	internal: bool,
	/// The number of IDs to generate, one per line.
	#[clap(short='n', long="number", default_value="1")]
	number: usize,
}

fn main() {
    let args = Args::parse();
	if !args.module && !args.port_type {
		eprintln!("error: must specify at least one of --module or --port-type");
		std::process::exit(1);
	}

	if args.module && args.port_type {
		eprintln!("error: cannot specify both --module and --port-type");
		std::process::exit(1);
	}

	let mut rng = thread_rng();

	for _ in 0..args.number {
		let high = if args.internal {
			0
		} else {
			rng.gen::<u64>() & 0x1FFF_FFFF_FFFF_FFFF
		};
		let low = rng.gen::<u64>();

		if args.module {
			println!("{}", Id::<{ oro_id::IdType::Module }>::from_high_low(high, low));
		} else if args.port_type {
			println!("{}", Id::<{ oro_id::IdType::PortType }>::from_high_low(high, low));
		} else {
			unreachable!()
		}
	}
}
