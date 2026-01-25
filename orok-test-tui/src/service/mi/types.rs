use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[allow(unused)]
pub enum MiResponse {
	Done,
	ThreadGroupAdded {
		id: String,
	},
	#[serde(rename_all = "kebab-case")]
	Stopped {
		reason:         String,
		disp:           Option<String>,
		bkptno:         Option<u64>,
		signal_name:    Option<String>,
		signal_meaning: Option<String>,
		frame:          Option<Frame>,
		thread_id:      u64,
	},
	#[serde(rename_all = "kebab-case")]
	ThreadSelected {
		id:    u64,
		frame: Frame,
	},
	#[serde(rename_all = "kebab-case")]
	Running {
		thread_id: Option<String>,
	},
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[allow(unused)]
pub struct Frame {
	pub addr:     String,
	pub func:     String,
	pub args:     Vec<FrameArg>,
	pub arch:     String,
	pub file:     Option<String>,
	pub fullname: Option<String>,
	pub line:     Option<usize>,
	pub level:    Option<usize>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[allow(unused)]
pub struct FrameArg {
	name:  String,
	value: String,
}
