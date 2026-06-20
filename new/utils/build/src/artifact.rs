use std::collections::{HashMap, HashSet};

use clap::builder::PossibleValue;

use crate::vfs::Artifact;

#[derive(Clone)]
pub struct Parser {
	selections: HashMap<String, Vec<Artifact>>,
	possible_values: Vec<PossibleValue>,
}

impl Parser {
	pub fn new() -> Self {
		let mut selections = HashMap::<String, Vec<Artifact>>::new();

		let vfs = crate::vfs::Vfs::new_from_cargo();

		let mut possible_archs = Vec::new();
		let mut possible_components = Vec::new();
		let mut possible_artifacts = Vec::new();
		let mut seen_archs = HashSet::new();
		let mut seen_components = HashSet::new();

		for artifact in vfs.artifacts() {
			if seen_archs.insert(artifact.architecture) {
				possible_archs.push(
					PossibleValue::new(artifact.architecture.to_string())
						.help(artifact.architecture.description()),
				);
			}

			if seen_components.insert(artifact.component) {
				possible_components.push(
					PossibleValue::new(artifact.component.to_string())
						.help(artifact.component.description()),
				);
			}

			let pa = PossibleValue::new(artifact.name.clone());
			possible_artifacts.push(
				if let Some(description) = &artifact.description {
					pa.help(description.clone())
				} else {
					pa
				},
			);

			selections
				.entry(artifact.name.clone())
				.or_default()
				.push(artifact.clone());

			selections
				.entry(artifact.architecture.to_string())
				.or_default()
				.push(artifact.clone());
			selections
				.entry(artifact.component.to_string())
				.or_default()
				.push(artifact);
		}

		Self {
			selections,
			possible_values: possible_archs
				.into_iter()
				.chain(possible_components)
				.chain(possible_artifacts)
				.collect(),
		}
	}
}

impl clap::builder::TypedValueParser for Parser {
	type Value = Vec<Artifact>;

	fn parse_ref(
		&self,
		_cmd: &clap::Command,
		_arg: Option<&clap::Arg>,
		value: &std::ffi::OsStr,
	) -> Result<Self::Value, clap::Error> {
		let key = value.to_string_lossy().to_string();
		if let Some(selection) = self.selections.get(&key) {
			Ok(selection.clone())
		} else {
			Err(clap::Error::raw(
				clap::error::ErrorKind::InvalidValue,
				format!("invalid artifact selection: {}\n", key),
			))
		}
	}

	fn possible_values(&self) -> Option<Box<dyn Iterator<Item = PossibleValue> + '_>> {
		Some(Box::new(self.possible_values.iter().cloned()))
	}
}

pub trait ArtifactExt {
	fn collapse(self) -> Vec<Artifact>;
}

impl ArtifactExt for Vec<Vec<Artifact>> {
	fn collapse(self) -> Vec<Artifact> {
		let mut seen = HashSet::new();
		let collapsed: Vec<Artifact> = self
			.into_iter()
			.flatten()
			.filter(|artifact| seen.insert(artifact.name.clone()))
			.collect();

		if collapsed.is_empty() {
			// Collect all artifacts if no selection was made.
			crate::vfs::Vfs::new_from_cargo().artifacts().collect()
		} else {
			collapsed
		}
	}
}
