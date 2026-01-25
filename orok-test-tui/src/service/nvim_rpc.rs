use std::{
	path::{Path, PathBuf},
	sync::Arc,
	time::Duration,
};

// The nvim crate docs are a bit dry; use this for information:
// https://neovim.io/doc/user/api.html
use anyhow::{Context, Result, bail};
use nvim_rs::{
	Buffer, Neovim, Window,
	rpc::{IntoVal, handler::Dummy},
};
use tokio::{
	net::{UnixStream, unix::OwnedWriteHalf},
	sync::mpsc::Receiver,
};
use tokio_util::compat::{Compat, TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

const HANDSHAKE_TEXT: &str = "ORO_KERNEL_TUI";

const _: () = {
	// See https://docs.rs/nvim-rs/0.9.2/nvim_rs/neovim/struct.Neovim.html#method.handshake
	assert!(HANDSHAKE_TEXT.len() < 20 || HANDSHAKE_TEXT.len() > 31);
};

type W = Compat<OwnedWriteHalf>;

pub enum Event {
	Ready,
	OpenMainFile {
		filename: PathBuf,
	},
	OpenAuxFile {
		filename: PathBuf,
	},
	CloseAuxFile,
	/// 1-based line
	SetMainHighlight {
		line: Option<usize>,
	},
	/// 1-based line
	SetAuxHighlight {
		line: Option<usize>,
	},
}

pub async fn run(
	_bus: Arc<crate::Bus>,
	mut rx: Receiver<Event>,
	nvim_path: impl AsRef<Path>,
) -> Result<!> {
	// Wait for the ready signal
	let Some(Event::Ready) = rx.recv().await else {
		bail!("first message to nvim_rpc must be Ready event");
	};

	// Wait for the socket to come online.
	wait_for_socket(nvim_path.as_ref())
		.await
		.context("waiting for nvim socket failed")?;

	// Connect to the nvim instance
	let sock = UnixStream::connect(nvim_path.as_ref())
		.await
		.context("failed to connect to nvim listener")?;

	log::debug!("connected to nvim at {}", nvim_path.as_ref().display());

	let (reader, writer) = sock.into_split();

	let (client, nvim_fut) = Neovim::handshake(
		reader.compat(),
		writer.compat_write(),
		Dummy::new(),
		HANDSHAKE_TEXT,
	)
	.await
	.context("failed to establish connection with Neovim")?;

	let mut nvim_join = tokio::spawn(nvim_fut);

	let mut controller = NvimController::new(&client);

	loop {
		tokio::select! {
			res = &mut nvim_join => {
				bail!("nvim rpc channel driver exited unexpectedly: {res:?}");
			}

			res = rx.recv() => {
				let Some(evt) = res else {
					bail!("EOF");
				};

				match evt {
					Event::Ready => {
						log::warn!("nvim_rpc received Ready event after startup; ignoring");
					}
					Event::OpenMainFile { filename } => {
						log::trace!("opening main file in nvim: {filename:?}");
						controller.set_main_file(filename).await?;
					}
					Event::OpenAuxFile { filename } => {
						log::trace!("opening aux file in nvim: {filename:?}");
						controller.set_aux_file(filename).await?;
					}
					Event::CloseAuxFile => {
						log::trace!("closing aux file");
						controller.close_aux_window().await?;
					}
					Event::SetMainHighlight { line } => {
						log::trace!("setting main window highlight to: {line:?}");
						controller.set_main_highlight(line).await?;
					}
					Event::SetAuxHighlight { line } => {
						log::trace!("setting aux window highlight to: {line:?}");
						controller.set_aux_highlight(line).await?;
					}
				}
			}
		}
	}
}

async fn wait_for_socket(nvim_path: impl AsRef<Path>) -> Result<()> {
	for i in 0..10 {
		match tokio::fs::metadata(nvim_path.as_ref()).await {
			Ok(_) => return Ok(()),
			Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
				log::trace!("nvim RPC socket file not found; attempt {i} (waiting 100ms)...");
				tokio::time::sleep(Duration::from_millis(100)).await;
			}
			Err(e) => {
				bail!(
					"failed to get metadata for nvim socket: {e}: {}",
					nvim_path.as_ref().display()
				);
			}
		}
	}

	bail!("timed out trying to wait for neovim socket");
}

struct NvimController<'a> {
	client:      &'a Neovim<W>,
	main_window: Option<Window<W>>,
	aux_window:  Option<Window<W>>,
	main_ns:     Option<i64>,
	aux_ns:      Option<i64>,
}

impl<'a> NvimController<'a> {
	fn new(client: &'a Neovim<W>) -> Self {
		Self {
			client,
			main_window: None,
			aux_window: None,
			main_ns: None,
			aux_ns: None,
		}
	}

	async fn get_main_window(&mut self) -> Result<Window<W>> {
		if let Some(w) = &self.main_window {
			Ok(w.clone())
		} else {
			let w = self
				.client
				.get_current_win()
				.await
				.context("failed to get main window for first time")?;
			self.main_window = Some(w.clone());
			Ok(w)
		}
	}

	async fn get_main_buf(&mut self) -> Result<Buffer<W>> {
		let w = self.get_main_window().await?;
		w.get_buf()
			.await
			.context("failed to get main window buffer")
	}

	async fn get_aux_window(&mut self) -> Result<Window<W>> {
		if let Some(w) = &self.aux_window {
			Ok(w.clone())
		} else {
			let main_window = &self.get_main_window().await?;

			let buf = self
				.client
				.create_buf(true, false)
				.await
				.context("failed to create aux window buffer")?;

			let w = self
				.client
				.open_win(
					&buf,
					false,
					vec![
						("split".into(), "right".into()),
						("win".into(), main_window.into_val()),
					],
				)
				.await
				.context("failed to create aux window")?;

			self.aux_window = Some(w.clone());

			Ok(w)
		}
	}

	async fn get_aux_buf(&mut self) -> Result<Buffer<W>> {
		let w = self.get_aux_window().await?;
		w.get_buf().await.context("failed to get aux window buffer")
	}

	async fn get_main_ns(&mut self) -> Result<i64> {
		if let Some(i) = self.main_ns {
			Ok(i)
		} else {
			let i = self.create_ns("orok_tui_main").await?;
			self.main_ns = Some(i);
			Ok(i)
		}
	}

	async fn get_aux_ns(&mut self) -> Result<i64> {
		if let Some(i) = self.aux_ns {
			Ok(i)
		} else {
			let i = self.create_ns("orok_tui_aux").await?;
			self.aux_ns = Some(i);
			Ok(i)
		}
	}

	async fn create_ns(&mut self, name: &str) -> Result<i64> {
		let i = self
			.client
			.create_namespace(name)
			.await
			.context("failed to create nvim namespace")?;

		self.client
			.set_hl(
				i,
				"TuiLineHighlight",
				vec![
					("bg".into(), "#160404".into()),
					("force".into(), true.into()),
				],
			)
			.await
			.context("failed to create TuiLineHighlight hl class for ns")?;

		self.client
			.set_hl(
				i,
				"TuiSignHighlight",
				vec![
					("fg".into(), "#ff0000".into()),
					("bold".into(), true.into()),
					("force".into(), true.into()),
				],
			)
			.await
			.context("failed to create TuiLineHighlight hl class for ns")?;

		Ok(i)
	}

	async fn set_main_file(&mut self, filename: impl AsRef<Path>) -> Result<()> {
		let main_window = self.get_main_window().await?;

		self.client
			.set_current_win(&main_window)
			.await
			.context("failed to enter main window")?;
		self.client
			.command(&format!("edit! {}", filename.as_ref().display()))
			.await
			.context("failed to load file into main window")?;

		let main_buf = self.get_main_buf().await?;
		main_buf
			.set_option("modifiable", false.into())
			.await
			.context("failed to set modifiable=false for main buffer")?;
		main_buf
			.set_option("readonly", true.into())
			.await
			.context("failed to set readonly=true for main buffer")?;

		Ok(())
	}

	async fn set_aux_file(&mut self, filename: impl AsRef<Path>) -> Result<()> {
		let aux_window = self.get_aux_window().await?;

		self.client
			.set_current_win(&aux_window)
			.await
			.context("failed to enter aux window")?;
		self.client
			.command(&format!("edit! {}", filename.as_ref().display()))
			.await
			.context("failed to load file into aux window")?;

		let aux_buf = self.get_aux_buf().await?;
		aux_buf
			.set_option("modifiable", false.into())
			.await
			.context("failed to set modifiable=false for aux buffer")?;
		aux_buf
			.set_option("readonly", true.into())
			.await
			.context("failed to set readonly=true for aux buffer")?;

		Ok(())
	}

	async fn close_aux_window(&mut self) -> Result<()> {
		if self.aux_window.is_some() {
			let aux_window = self.get_aux_window().await?;
			let aux_buf = self.get_aux_buf().await?;

			aux_window
				.close(true)
				.await
				.context("failed to close aux window")?;

			aux_buf
				.delete(vec![("unload".into(), true.into())])
				.await
				.context("failed to delete aux buffer")?;

			self.aux_window = None;
		}

		Ok(())
	}

	async fn set_main_highlight(&mut self, line: Option<usize>) -> Result<()> {
		let main_window = self.get_main_window().await?;
		let main_buf = self.get_main_buf().await?;
		let ns = self.get_main_ns().await?;
		self.set_highlight(&main_window, &main_buf, ns, line).await
	}

	async fn set_aux_highlight(&mut self, line: Option<usize>) -> Result<()> {
		let aux_window = self.get_aux_window().await?;
		let aux_buf = self.get_aux_buf().await?;
		let ns = self.get_aux_ns().await?;
		self.set_highlight(&aux_window, &aux_buf, ns, line).await
	}

	async fn set_highlight(
		&mut self,
		window: &Window<W>,
		buf: &Buffer<W>,
		ns: i64,
		line: Option<usize>,
	) -> Result<()> {
		buf.clear_namespace(ns, 0, -1)
			.await
			.context("failed to clear namespace")?;

		window
			.set_hl_ns(ns)
			.await
			.context("failed to set window namespace")?;

		if let Some(line) = line {
			buf.set_extmark(
				ns,
				(line - 1) as i64,
				0,
				vec![
					("end_row".into(), (line - 1).into()),
					("line_hl_group".into(), "TuiLineHighlight".into()),
					("sign_text".into(), "->".into()),
					("sign_hl_group".into(), "TuiSignHighlight".into()),
				],
			)
			.await
			.context("failed to set buffer highlight")?;

			window
				.set_cursor((line as i64, 0))
				.await
				.context("failed to set the window cursor after setting the line")?;
		}

		Ok(())
	}
}
