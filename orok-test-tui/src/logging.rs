use std::sync::{
	Mutex, OnceLock,
	atomic::{AtomicBool, Ordering::Relaxed},
};

use ringbuf::{
	SharedRb,
	storage::Heap,
	traits::{Consumer, RingBuffer},
};
use tokio::sync::Notify;

const BUCKET_SIZE: usize = 1024;

static LOGGER: LogState = LogState::new();

struct LogState {
	messages:      OnceLock<Mutex<LogBuckets>>,
	notifier:      Notify,
	trace_enabled: AtomicBool,
	debug_enabled: AtomicBool,
	info_enabled:  AtomicBool,
	warn_enabled:  AtomicBool,
	error_enabled: AtomicBool,
}

struct LogBuckets {
	counter: u64,
	trace:   SharedRb<Heap<LogRecord>>,
	debug:   SharedRb<Heap<LogRecord>>,
	info:    SharedRb<Heap<LogRecord>>,
	warn:    SharedRb<Heap<LogRecord>>,
	error:   SharedRb<Heap<LogRecord>>,
}

impl LogBuckets {
	fn new() -> Self {
		Self {
			counter: 1, // must not be 0
			trace:   SharedRb::new(BUCKET_SIZE),
			debug:   SharedRb::new(BUCKET_SIZE),
			info:    SharedRb::new(BUCKET_SIZE),
			warn:    SharedRb::new(BUCKET_SIZE),
			error:   SharedRb::new(BUCKET_SIZE),
		}
	}
}

impl LogState {
	const fn new() -> Self {
		Self {
			messages:      OnceLock::new(),
			notifier:      Notify::const_new(),
			trace_enabled: AtomicBool::new(false),
			debug_enabled: AtomicBool::new(true),
			info_enabled:  AtomicBool::new(true),
			warn_enabled:  AtomicBool::new(true),
			error_enabled: AtomicBool::new(true),
		}
	}
}

pub fn init_logger() -> Result<(), log::SetLoggerError> {
	log::set_logger(&LOGGER)?;
	Ok(())
}

pub async fn wait_for_log() {
	LOGGER.notifier.notified().await;
}

pub fn for_each_log_reverse<F>(mut f: F)
where
	F: FnMut(&LogRecord) -> bool,
{
	let messages = LOGGER
		.messages
		.get_or_init(|| Mutex::new(LogBuckets::new()))
		.lock()
		.unwrap();

	let mut iters = Vec::with_capacity(5);
	if LOGGER.error_enabled.load(Relaxed) {
		iters.push(messages.error.iter().rev().peekable());
	}
	if LOGGER.warn_enabled.load(Relaxed) {
		iters.push(messages.warn.iter().rev().peekable());
	}
	if LOGGER.info_enabled.load(Relaxed) {
		iters.push(messages.info.iter().rev().peekable());
	}
	if LOGGER.debug_enabled.load(Relaxed) {
		iters.push(messages.debug.iter().rev().peekable());
	}
	if LOGGER.trace_enabled.load(Relaxed) {
		iters.push(messages.trace.iter().rev().peekable());
	}

	loop {
		let mut max_counter = 0;
		let mut max_level = 0;
		for (level, iter) in iters.iter_mut().enumerate() {
			if let Some(record) = iter.peek()
				&& record.id > max_counter
			{
				max_counter = record.id;
				max_level = level;
			}
		}

		if max_counter == 0 {
			break;
		}
		let record = iters[max_level].next().unwrap();
		if !f(record) {
			break;
		}
	}
}

pub struct LogRecord {
	id:          u64,
	pub level:   log::Level,
	pub target:  String,
	pub message: String,
}

impl log::Log for LogState {
	fn enabled(&self, _metadata: &log::Metadata) -> bool {
		true
	}

	fn log(&self, record: &log::Record) {
		let mut messages = self
			.messages
			.get_or_init(|| Mutex::new(LogBuckets::new()))
			.lock()
			.unwrap();

		let message = format!("{}", record.args());

		let mut counter = messages.counter;

		let bucket = match record.level() {
			log::Level::Trace => &mut messages.trace,
			log::Level::Debug => &mut messages.debug,
			log::Level::Info => &mut messages.info,
			log::Level::Warn => &mut messages.warn,
			log::Level::Error => &mut messages.error,
		};

		for line in message.lines() {
			bucket.push_overwrite(LogRecord {
				id:      counter,
				level:   record.level(),
				target:  record.target().to_string(),
				message: line.to_string(),
			});
			counter += 1;
		}

		messages.counter = counter;
		drop(messages);

		// Only notify if the current log line is enabled
		// otherwise the TUI refreshes for every single little
		// log line.
		if is_enabled(record.level()) {
			self.notifier.notify_waiters();
		}
	}

	fn flush(&self) {
		// no-op
	}
}

pub fn toggle_level(level: log::Level) -> bool {
	match level {
		log::Level::Trace => LOGGER.trace_enabled.fetch_xor(true, Relaxed),
		log::Level::Debug => LOGGER.debug_enabled.fetch_xor(true, Relaxed),
		log::Level::Info => LOGGER.info_enabled.fetch_xor(true, Relaxed),
		log::Level::Warn => LOGGER.warn_enabled.fetch_xor(true, Relaxed),
		log::Level::Error => LOGGER.error_enabled.fetch_xor(true, Relaxed),
	}
}

pub fn is_enabled(level: log::Level) -> bool {
	match level {
		log::Level::Trace => LOGGER.trace_enabled.load(Relaxed),
		log::Level::Debug => LOGGER.debug_enabled.load(Relaxed),
		log::Level::Info => LOGGER.info_enabled.load(Relaxed),
		log::Level::Warn => LOGGER.warn_enabled.load(Relaxed),
		log::Level::Error => LOGGER.error_enabled.load(Relaxed),
	}
}
