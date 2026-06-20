use std::{
	collections::{HashMap, HashSet},
	path::PathBuf,
};

pub struct Vfs {
	root_dir: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Deserialize)]
pub enum Arch {
	#[serde(rename = "x86_64")]
	X64,
	#[serde(rename = "aarch64")]
	Aarch64,
	#[serde(rename = "riscv64")]
	Riscv64,
}

impl Arch {
	pub fn description(&self) -> &'static str {
		match self {
			Self::X64 => "All x86_64 artifacts",
			Self::Aarch64 => "All AArch64 artifacts",
			Self::Riscv64 => "All RISC-V 64 artifacts",
		}
	}
}

impl std::fmt::Display for Arch {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::X64 => write!(f, "x86_64"),
			Self::Aarch64 => write!(f, "aarch64"),
			Self::Riscv64 => write!(f, "riscv64"),
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Component {
	Kernel,
	Bootloader,
}

impl std::fmt::Display for Component {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Kernel => write!(f, "kernel"),
			Self::Bootloader => write!(f, "bootloader"),
		}
	}
}

impl Component {
	pub fn description(&self) -> &'static str {
		match self {
			Self::Kernel => "All kernel artifacts",
			Self::Bootloader => "All bootloader artifacts",
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LockEntry {
	pub package_name: String,
	pub is_registry: bool,
	pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Artifact {
	pub path: PathBuf,
	pub name: String,
	pub package_name: String,
	pub description: Option<String>,
	pub architecture: Arch,
	pub component: Component,
	pub target_relative_path: PathBuf,
	pub target_triple: String,
}

#[derive(serde::Deserialize)]
struct CargoManifest {
	package: CargoPackage,
}

#[derive(serde::Deserialize)]
struct CargoPackage {
	name: String,
	description: Option<String>,
	metadata: CargoPackageMetadata,
}

#[derive(serde::Deserialize)]
struct CargoPackageMetadata {
	oro: CargoPackageMetadataOro,
}

#[derive(serde::Deserialize)]
struct CargoPackageMetadataOro {
	arch: Arch,
	component: Component,
}

#[derive(serde::Deserialize)]
struct CargoLockfile {
	version: u32,
	package: Vec<CargoLockfilePackage>,
}

#[derive(serde::Deserialize)]
struct CargoLockfilePackage {
	name: String,
	source: Option<String>,
	dependencies: Option<Vec<String>>,
}

impl Vfs {
	pub fn new(root_dir: impl Into<PathBuf>) -> Self {
		Self {
			root_dir: root_dir.into(),
		}
	}

	/// # Panics
	/// Panics if the `CARGO_MANIFEST_DIR` environment variable is not set.
	pub fn new_from_cargo() -> Self {
		Self::new(
			PathBuf::from(
				std::env::var("CARGO_MANIFEST_DIR")
					.expect("CARGO_MANIFEST_DIR environment variable not set"),
			)
			.join("../../"),
		)
	}

	pub fn artifacts(&self) -> impl Iterator<Item = Artifact> {
		self.read_artifact_dir().filter_map(|path| {
			let manifest = std::fs::read_to_string(path.join("Cargo.toml")).ok()?;
			let manifest: CargoManifest = toml::from_str(&manifest).ok()?;
			let target_triple = format!("{}-unknown-oro", manifest.package.metadata.oro.arch);
			Some(Artifact {
				path,
				name: format!(
					"{}-{}",
					manifest.package.metadata.oro.arch, manifest.package.metadata.oro.component
				),
				description: manifest.package.description,
				architecture: manifest.package.metadata.oro.arch,
				component: manifest.package.metadata.oro.component,
				target_relative_path: PathBuf::from(&manifest.package.name),
				package_name: manifest.package.name,
				target_triple,
			})
		})
	}

	/// # Panics
	/// Panics if the artifacts directory cannot be read.
	fn read_artifact_dir(&self) -> impl Iterator<Item = PathBuf> {
		std::fs::read_dir(self.root_dir.join("artifact"))
			.expect(&format!(
				"failed to read artifacts directory: {}",
				self.root_dir.join("artifact").display()
			))
			.filter_map(|entry| entry.ok())
			.filter(|entry| entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
			.map(|entry| entry.path())
	}

	/// # Panics
	/// Panics if the lockfile cannot be read.
	pub fn lockfile(&self) -> impl Iterator<Item = LockEntry> {
		let contents = std::fs::read_to_string(self.root_dir.join("Cargo.lock"))
			.expect("failed to read Cargo.lock");
		let lockfile: CargoLockfile =
			toml::from_str(&contents).expect("failed to parse Cargo.lock");
		assert_eq!(
			lockfile.version, 4,
			"Cargo.lock version 4 is the only version supported"
		);
		lockfile.package.into_iter().map(|e| {
			LockEntry {
				package_name: e.name,
				is_registry: e.source.is_some_and(|s| s.starts_with("registry+")),
				dependencies: e.dependencies.unwrap_or_default(),
			}
		})
	}

	/// Attempts to parse the `.cargo/config.toml` at the root
	/// of the repository as a `toml::Value`.
	///
	/// # Panics
	/// Panics if it cannot read the file.
	pub fn config_toml(&self) -> toml::Value {
		let contents = std::fs::read_to_string(self.root_dir.join(".cargo").join("config.toml"))
			.expect("failed to read .cargo/config.toml");
		toml::from_str(&contents).expect("failed to parse .cargo/config.toml")
	}

	/// Attempts to write the given `toml::Value` to `.cargo/config.toml`
	/// at the root of the repository.
	///
	/// # Panics
	/// Panics if it cannot write the file.
	pub fn write_config_toml(&self, value: toml::Value) {
		let contents = toml::to_string_pretty(&value)
			.expect("failed to generate .cargo/config.toml TOML string");
		std::fs::write(
			self.root_dir.join(".cargo").join("config.toml"),
			contents.as_bytes(),
		)
		.expect("failed to write .cargo/config.toml");
	}
}

pub trait Lockfile: IntoIterator<Item = LockEntry> + Sized {
	fn into_map(self) -> HashMap<String, LockEntry> {
		self.into_iter()
			.map(|e| (e.package_name.clone(), e))
			.collect()
	}

	fn used(self) -> impl Iterator<Item = LockEntry>
	where
		Self: Clone,
	{
		let mut seen = HashSet::new();

		for item in self.clone().into_iter() {
			if item.is_registry {
				for dep in item.dependencies {
					seen.insert(dep);
				}
			} else {
				seen.insert(item.package_name);
			}
		}

		self.into_iter()
			.filter(move |e| seen.contains(&e.package_name))
	}

	fn unused(self) -> impl Iterator<Item = LockEntry>
	where
		Self: Clone,
	{
		let mut seen = HashSet::new();

		for item in self.clone().into_iter() {
			if item.is_registry {
				for dep in item.dependencies {
					seen.insert(dep);
				}
			} else {
				seen.insert(item.package_name);
			}
		}

		self.into_iter()
			.filter(move |e| !seen.contains(&e.package_name))
	}

	fn dependencies(self) -> impl Iterator<Item = LockEntry> {
		self.into_iter().filter(|e| e.is_registry)
	}
}

impl<T> Lockfile for T where T: IntoIterator<Item = LockEntry> {}

pub trait ConfigToml {
	fn as_toml(&self) -> &toml::Value;
	fn as_mut_toml(&mut self) -> &mut toml::Value;

	/// # Panics
	///
	/// Panics if the given `.cargo/config.toml` isn't a table.
	fn set_vendor_paths(&mut self, lockfile: impl Iterator<Item = LockEntry>) {
		let t = self.as_mut_toml();
		let toml::Value::Table(tbl) = t else {
			panic!(".cargo/config.toml is not a TOML table");
		};

		let new_paths = toml::Value::Array(
			lockfile
				.dependencies()
				.map(|e| toml::Value::String(format!("vendor/{}", e.package_name)))
				.collect(),
		);
		tbl.insert("paths".into(), new_paths);
	}
}

impl ConfigToml for toml::Value {
	fn as_toml(&self) -> &toml::Value {
		self
	}

	fn as_mut_toml(&mut self) -> &mut toml::Value {
		self
	}
}
