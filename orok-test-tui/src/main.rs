#![feature(never_type, if_let_guard, try_with_capacity, mapped_lock_guards)]

use std::{
	cell::RefCell,
	io::{self, Stdout},
	path::PathBuf,
	sync::Arc,
};

use anyhow::{Context, anyhow};
use crossterm::{
	event::{DisableMouseCapture, EnableMouseCapture},
	execute,
	terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

mod atomic;
mod logging;
mod pty;
mod service;

#[derive(Debug)]
pub enum AppEvent {
	/// Restores the terminal and quits the app.
	///
	/// Does its best to clean things up but you should probably call
	/// `ScramAndQuit` from the session service instead.
	Quit { status_code: i32 },
}

struct BusData {
	terminal:         Arc<RefCell<Terminal<ratatui::backend::CrosstermBackend<Stdout>>>>,
	video_path:       PathBuf,
	gdb_rsp_path:     PathBuf,
	qemu_rsp_path:    PathBuf,
	gdb_mi_path:      PathBuf,
	orodbg_path:      PathBuf,
	nvim_path:        PathBuf,
	nvim_config_path: PathBuf,
}

macro_rules! bus {
	($(
		$name:ident$((
			$( $(& $arg_ref:ident)? $($arg:ident)? ),*
		))? = $buf_count:literal
	),* $(,)?) => {
		pub struct Bus {
			#[allow(unused)]
			pub app: tokio::sync::mpsc::Sender<crate::AppEvent>,
			$(
				pub $name: tokio::sync::mpsc::Sender<self::service::$name::Event>,
			)*
		}

		impl Bus {
			async fn run(params: BusData) -> anyhow::Result<i32> {
				let (app_tx, mut app_rx) = tokio::sync::mpsc::channel(4);

				$(
					let $name;
				)*

				#[allow(unused)]
				let bus = Bus {
					app: app_tx,
					$(
						$name: {
							let (tx, rx) = tokio::sync::mpsc::channel($buf_count);
							$name = rx;
							tx
						},
					)*
				};

				#[allow(unused)]
				let bus = std::sync::Arc::new(bus);

				$(
					let $name = service::$name::run(
						Arc::clone(&bus),
						$name,
						$( $( $(& params.$arg_ref)? $(params.$arg)? ),* )?
					);
					futures::pin_mut!($name);
				)*

				Ok(loop {
					tokio::select! {
						$(
							res = &mut $name => {
								res?;
							}
						)*

						req = app_rx.recv() => match req.ok_or(anyhow!("AppEvent receiver closed unexpectedly"))? {
							AppEvent::Quit { status_code } => {
								break status_code;
							}
						}
					}
				})
			}
		}
	}
}

bus! {
	tui(terminal) = 1024,
	input = 2,
	logging = 2,
	video(&video_path) = 8,
	gdb_rsp(&gdb_rsp_path) = 32,
	qemu_rsp(&qemu_rsp_path) = 32,
	session = 4,
	gdb(&gdb_rsp_path, &gdb_mi_path) = 128,
	qemu(&qemu_rsp_path, &video_path, &orodbg_path) = 128,
	mi(&gdb_mi_path) = 128,
	orodbg_sock(&orodbg_path) = 2,
	orodbg = 2048,
	debounce = 32,
	nvim(&nvim_path, &nvim_config_path) = 128,
	nvim_rpc(&nvim_path) = 64,
}

#[::tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	log::set_max_level(log::LevelFilter::Trace);
	logging::init_logger().unwrap();

	let old_hook = std::panic::take_hook();
	std::panic::set_hook(Box::new(move |info| {
		ratatui::restore();
		old_hook(info);
	}));

	enable_raw_mode()?;
	let mut stdout = io::stdout();
	execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
	let backend = CrosstermBackend::new(stdout);
	#[expect(
		clippy::arc_with_non_send_sync,
		reason = "This is a hack to work around async terminal shenanigans."
	)]
	let terminal = Arc::new(RefCell::new(Terminal::new(backend)?));

	log::info!("Oro Operating System kernel Test TUI is online");

	let tmpdir =
		tempfile::tempdir().with_context(|| "failed to create temporary directory for servers")?;

	let result = Bus::run(BusData {
		terminal:         Arc::clone(&terminal),
		video_path:       tmpdir.path().join("video.sock"),
		gdb_rsp_path:     tmpdir.path().join("rsp.gdb.sock"),
		qemu_rsp_path:    tmpdir.path().join("rsp.qemu.sock"),
		gdb_mi_path:      tmpdir.path().join("mi.sock"),
		orodbg_path:      tmpdir.path().join("orodbg.sock"),
		nvim_path:        tmpdir.path().join("nvim.sock"),
		nvim_config_path: tmpdir.path().join("nvim.vim"),
	})
	.await;

	disable_raw_mode()?;
	// NOTE(qix-): https://github.com/crossterm-rs/crossterm/pull/1028
	{
		let mut term = terminal.borrow_mut();
		execute!(
			term.backend_mut(),
			LeaveAlternateScreen,
			DisableMouseCapture
		)?;
		term.show_cursor()?;
	}

	match result {
		Ok(0) => {
			log::info!("exiting with status code 0");
		}
		Ok(code) => {
			log::warn!("exiting with non-zero code {code}");
			std::process::exit(code);
		}
		Err(err) => {
			eprintln!("fatal error: {:?}", err);
			std::process::exit(1);
		}
	}

	Ok(())
}
