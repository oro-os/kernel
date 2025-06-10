#![expect(dead_code)]

use std::{collections::HashMap, path::PathBuf};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct BuildPlan {
	pub inputs:      Vec<PathBuf>,
	pub invocations: Vec<Invocation>,
}

#[derive(Debug, Deserialize)]
pub struct Invocation {
	pub args: Vec<String>,
	pub env: HashMap<String, String>,
	pub compile_mode: CompileMode,
	pub cwd: PathBuf,
	pub deps: Vec<usize>,
	/// Appears to be `--target`, if provided.
	pub kind: Option<String>,
	pub links: HashMap<PathBuf, PathBuf>,
	pub outputs: Vec<PathBuf>,
	pub package_name: String,
	pub package_version: String,
	pub program: String,
	pub target_kind: Vec<TargetKind>,
}

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "kebab-case")]
pub enum CompileMode {
	Build,
	RunCustomBuild,
}

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "kebab-case")]
pub enum TargetKind {
	Lib,
	Bin,
	CustomBuild,
	ProcMacro,
}
