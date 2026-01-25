use std::path::PathBuf;

/// Requests sent to the GDB service
#[derive(Debug)]
pub enum GdbRequest {
	/// Start GDB with the given arguments
	///
	/// Any existing session is first shut down
	Start {
		args: Vec<String>,
		rows: u16,
		cols: u16,
	},
	/// Write input to GDB
	WriteInput { bytes: Vec<u8> },
	/// Resize the PTY
	Resize { rows: u16, cols: u16 },
	/// Terminate GDB
	Shutdown,
	/// MI: Continues the GDB session
	MiContinue,
}

/// Requests sent to the GDB RSP service
#[derive(Debug)]
pub enum GdbRspRequest {
	/// Send RSP packet to GDB
	SendPacket { data: Vec<u8> },
	/// Shut down any existing connection
	Shutdown,
}

/// Requests sent to the QEMU RSP service
#[derive(Debug)]
pub enum QemuRspRequest {
	/// Send RSP packet to QEMU
	SendPacket { data: Vec<u8> },
	/// Shut down any existing connection
	Shutdown,
}

/// Requests sent to the QEMU Video service
#[derive(Debug)]
pub enum VideoRequest {
	/// Shut down any existing connection
	Shutdown,
}

/// Requests sent to the session service
#[derive(Debug)]
pub enum SessionRequest {
	/// Start a session
	Start {
		qemu_rsp_path: PathBuf,
		qemu_video_path: PathBuf,
		makefile_job: String,
		rows: u16,
		cols: u16,
	},
	/// Shutdown any running session
	Shutdown,
	/// Resize the Session TUI
	Resize { rows: u16, cols: u16 },
	/// Write input to the session's PTY
	WriteInput { bytes: Vec<u8> },
}

/// Responses from the GDB service
#[derive(Debug)]
pub enum AppEvent {
	/// GDB instance has spawned
	GdbSpawned,
	/// Stdout from GDB
	GdbStdout { bytes: Vec<u8> },
	/// GDB has exited (if `None`, exit was caused by a closed pipe)
	GdbExited { code: Option<u32> },
	/// Video connection established
	VideoConnected,
	/// Video connection lost
	VideoDisconnected,
	/// New video frame received
	VideoFrame { image: image::DynamicImage },
	/// Video frame size changed
	VideoSizeChanged { width: u64, height: u64 },
	/// QEMU RSP connection established
	QemuRspConnected,
	/// QEMU RSP connection lost
	QemuRspDisconnected,
	/// QEMU RSP packet received
	QemuRspPacket { data: Vec<u8> },
	/// GDB RSP connection established
	GdbRspConnected,
	/// GDB RSP connection lost
	GdbRspDisconnected,
	/// GDB RSP packet received
	GdbRspPacket { data: Vec<u8> },
	/// The logger was updated with new messages since the last tick
	LogUpdated,
	/// The GDB TUI resized
	GdbTuiResized { cols: u16, rows: u16 },
	/// User requested to start an x86_64 session.
	StartX8664Session,
	/// User requested to start an AArch64 session.
	StartAarch64Session,
	/// Scram event from TUI (shuts down all sessions)
	Scram,
	/// Session has started
	SessionStarted,
	/// Session has stopped
	SessionStopped { code: Option<u32> },
	/// GDB RSP packet received
	SessionStdout { data: Vec<u8> },
	/// The Session TUI resized
	SessionTuiResized { cols: u16, rows: u16 },
	/// Continue the GDB session
	GdbMiContinue,
}

/// Multiplexed events that the main event loop can consume
#[derive(Debug)]
pub enum Event {
	App(AppEvent),
	Tui(crossterm::event::Event),
}
