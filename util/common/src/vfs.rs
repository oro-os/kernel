use std::{
	collections::{HashMap, HashSet, VecDeque},
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

/// The cargo profile an artifact is built under.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Profile {
	Dev,
	Release,
}

impl Profile {
	/// The name of the profile's output directory under `target/<triple>/`.
	pub fn target_dir_name(&self) -> &'static str {
		match self {
			Self::Dev => "debug",
			Self::Release => "release",
		}
	}
}

impl std::fmt::Display for Profile {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Dev => write!(f, "dev"),
			Self::Release => write!(f, "release"),
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Artifact {
	pub path: PathBuf,
	pub name: String,
	pub package_name: String,
	pub description: Option<String>,
	pub architecture: Arch,
	pub component: Component,
	/// The artifact's kind, taken from its directory name with the
	/// architecture prefix stripped (e.g. `artifact/x86_64-limine`
	/// has kind `limine`). Unlike [`Component`], this distinguishes
	/// between different implementations of the same component
	/// (e.g. multiple bootloaders).
	pub kind: String,
	/// The name of the produced binary; usually the package name, but
	/// honors `[[bin]]` renames (e.g. `oro-limine-x86-64`, hyphenated
	/// to match Limine's `ARCH` naming).
	pub binary_name: String,
	pub target_relative_path: PathBuf,
	pub target_triple: String,
}

impl Artifact {
	/// The path to the artifact's built binary for the given profile,
	/// relative to the workspace root.
	///
	/// Note that this does not imply the binary exists; the artifact
	/// must have been built (e.g. via `cargo oro build`) first.
	pub fn binary_path(&self, workspace_root: &std::path::Path, profile: Profile) -> PathBuf {
		workspace_root
			.join("target")
			.join(&self.target_triple)
			.join(profile.target_dir_name())
			.join(&self.binary_name)
	}
}

#[derive(serde::Deserialize)]
pub struct CargoManifest {
	pub package: CargoPackage,
	pub bin: Option<Vec<CargoManifestBin>>,
	pub dependencies: Option<HashMap<String, CargoManifestDependency>>,
}

#[derive(serde::Deserialize)]
pub struct CargoManifestBin {
	pub name: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(untagged)]
pub enum CargoManifestDependency {
	Simple(String),
	Detailed(CargoManifestDependencyDetailed),
}

impl CargoManifestDependency {
	pub fn version(&self) -> Option<&str> {
		match self {
			Self::Simple(v) => Some(v.as_str()),
			Self::Detailed(d) => d.version.as_deref(),
		}
	}
}

#[derive(serde::Deserialize)]
pub struct CargoManifestDependencyDetailed {
	pub version: Option<String>,
}

#[derive(serde::Deserialize)]
pub struct CargoPackage {
	pub name: String,
	pub version: String,
	pub description: Option<String>,
	pub license: Option<String>,
	pub license_file: Option<String>,
	pub metadata: Option<CargoPackageMetadata>,
}

#[derive(serde::Deserialize)]
pub struct CargoPackageMetadata {
	pub oro: Option<CargoPackageMetadataOro>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CargoPackageMetadataOro {
	pub arch: Option<Arch>,
	pub component: Option<Component>,
	pub license_overrides: Option<HashMap<String, CargoPackageMetadataLicenseOverride>>,
}

#[derive(serde::Deserialize)]
pub struct CargoPackageMetadataLicenseOverride {
	#[serde(rename = "path")]
	pub relative_path: PathBuf,
}

#[derive(serde::Deserialize)]
pub struct CargoLockfile {
	pub version: u32,
	pub package: Vec<CargoLockfilePackage>,
}

#[derive(serde::Deserialize)]
pub struct CargoLockfilePackage {
	pub name: String,
	pub source: Option<String>,
	pub dependencies: Option<Vec<String>>,
}

pub struct VendorPackage {
	pub root_path: PathBuf,
	pub license_path: Option<PathBuf>,
	pub manifest: CargoManifest,
}

impl VendorPackage {
	pub fn override_license_info(
		&mut self,
		overrides: &HashMap<String, CargoPackageMetadataLicenseOverride>,
	) {
		let Some(entry) = overrides.get(&self.manifest.package.name) else {
			return;
		};
		self.license_path = Some(self.root_path.join(&entry.relative_path));
	}
}

#[derive(Debug)]
pub struct LicenseLink {
	pub license_path: PathBuf,
	pub target_path: PathBuf,
	pub is_symlink: bool,
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

	pub fn root(&self) -> PathBuf {
		self.root_dir.clone()
	}

	pub fn artifacts(&self) -> impl Iterator<Item = Artifact> {
		self.read_artifact_dir().filter_map(|path| {
			let manifest = std::fs::read_to_string(path.join("Cargo.toml")).ok()?;
			let manifest: CargoManifest = toml::from_str(&manifest).ok()?;
			let metadata = &manifest.package.metadata.as_ref()?.oro.as_ref()?;
			let arch = metadata.arch?;
			let component = metadata.component?;
			let target_triple = format!("{}-unknown-oro", arch);
			let kind = path
				.file_name()
				.map(|n| n.to_string_lossy())
				.and_then(|n| n.strip_prefix(&format!("{arch}-")).map(str::to_string))
				.unwrap_or_else(|| component.to_string());
			let binary_name = manifest
				.bin
				.as_ref()
				.and_then(|bins| bins.first())
				.and_then(|bin| bin.name.clone())
				.unwrap_or_else(|| manifest.package.name.clone());
			Some(Artifact {
				path,
				name: format!("{arch}-{component}"),
				description: manifest.package.description,
				architecture: arch,
				component,
				kind,
				binary_name,
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
	/// Panics if the root `Cargo.toml` cannot be read.
	pub fn root_cargo_toml(&self) -> CargoManifest {
		let contents = std::fs::read_to_string(self.root_dir.join("Cargo.toml"))
			.expect("failed to read root Cargo.toml");
		toml::from_str(&contents).expect("failed to parse root Cargo.toml")
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
		let root_package_name = self.root_cargo_toml().package.name;
		lockfile.package.into_iter().filter_map(move |e| {
			(e.name != root_package_name).then(|| {
				LockEntry {
					package_name: e.name,
					is_registry: e.source.is_some_and(|s| s.starts_with("registry+")),
					dependencies: e.dependencies.unwrap_or_default(),
				}
			})
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

	/// Reads the vendor packages directory
	///
	/// # Panics
	/// Panics if the vendor directory cannot be read.
	pub fn vendor_packages(&self) -> impl Iterator<Item = VendorPackage> {
		std::fs::read_dir(self.root_dir.join("vendor"))
			.expect(&format!(
				"failed to read vendor directory: {}",
				self.root_dir.join("vendor").display()
			))
			.filter_map(|entry| entry.ok())
			.filter(|entry| {
				entry.path().file_name().is_some_and(|n| {
					n.to_string_lossy()
						.bytes()
						.next()
						.is_some_and(|b| b != b'.')
				}) && entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false)
			})
			.map(|entry| {
				let manifest: CargoManifest = toml::from_str(
					&std::fs::read_to_string(entry.path().join("Cargo.toml")).expect(&format!(
						"failed to read Cargo.toml for vendor dependency: {}",
						entry.path().display()
					)),
				)
				.expect(&format!(
					"failed to parse Cargo.toml for vendor dependency: {}",
					entry.path().display()
				));

				VendorPackage {
					root_path: entry.path(),
					license_path: manifest
						.package
						.license_file
						.as_ref()
						.and_then(|lf| Some(entry.path().join(lf)))
						.or_else(|| entry.path().try_find_license()),
					manifest,
				}
			})
	}

	pub fn vendor_license_dir(&self) -> PathBuf {
		self.root_dir.join("license").join("vendor")
	}

	/// Reads out all of the third-party licenses.
	///
	/// # Panics
	/// Panics if it cannot read the `license/vendor` directory.
	pub fn vendor_licenses(&self) -> impl Iterator<Item = LicenseLink> {
		std::fs::read_dir(self.vendor_license_dir())
			.expect(&format!(
				"failed to read vendor license directory: {}",
				self.vendor_license_dir().display()
			))
			.filter_map(|entry| entry.ok())
			.filter(|entry| {
				entry
					.file_type()
					.map(|ft| ft.is_file() || ft.is_symlink())
					.unwrap_or(false)
			})
			.map(|entry| {
				LicenseLink {
					is_symlink: entry
						.file_type()
						.map(|ft| ft.is_symlink())
						.unwrap_or_default(),
					license_path: entry.path(),
					target_path: entry
						.file_type()
						.map(|ft| ft.is_symlink())
						.unwrap_or_default()
						.then(|| {
							std::fs::canonicalize(entry.path()).expect(&format!(
								"failed to canonicalize symlink license path: {}",
								entry.path().display()
							))
						})
						.unwrap_or_else(|| entry.path().into()),
				}
			})
	}
}

trait LicensePath {
	fn try_find_license(&self) -> Option<PathBuf>;
}

impl<P: AsRef<std::path::Path>> LicensePath for P {
	fn try_find_license(&self) -> Option<PathBuf> {
		let base = self.as_ref().to_path_buf();
		macro_rules! test_license_file {
			($($relpath:literal),* $(,)?) => {
				(None)$(.or_else(|| {let p = base.join($relpath); p.try_exists().unwrap_or_default().then(|| p)}))*
			}
		}

		test_license_file![
			"LICENSE-MIT",
			"LICENSE-APACHE",
			"license-mit",
			"license-apache",
			"license-apache-2.0",
			"LICENSE",
			"LICENSE.txt",
			"LICENSE.TXT",
			"LICENSE.md",
			"LICENSE.MD",
			"COPYING",
			"COPYING.txt",
			"COPYING.TXT",
			"COPYING.md",
			"COPYING.MD",
			"LICENSE.apache2",
			"LICENSE.apache",
			"LICENSE.mit",
			"LICENSE.gpl",
			"LICENSE.gpl2",
			"LICENSE.gpl-2.0",
			"LICENSE.gpl2.0",
			"LICENSE.gpl3",
			"LICENSE.gpl-3.0",
			"LICENSE.gpl2.0",
		]
	}
}

pub trait Lockfile: IntoIterator<Item = LockEntry> + Sized {
	fn into_map(self) -> HashMap<String, LockEntry> {
		self.into_iter()
			.map(|e| (e.package_name.clone(), e))
			.collect()
	}

	fn used(self) -> impl Iterator<Item = LockEntry> {
		let map = self.into_map();
		let mut seen = HashSet::new();
		let mut queue = map
			.iter()
			.filter_map(|(n, e)| (!e.is_registry).then(|| n.to_string()))
			.collect::<VecDeque<_>>();

		while let Some(name) = queue.pop_back() {
			if !seen.insert(name.clone()) {
				continue;
			}

			for dep in &map[&name].dependencies {
				queue.push_back(dep.into());
			}
		}

		map.into_iter()
			.filter_map(move |(k, e)| seen.contains(&k).then(move || e))
	}

	fn unused(self) -> impl Iterator<Item = LockEntry> {
		let all: Vec<LockEntry> = self.into_iter().collect();
		let used: HashSet<String> = all.iter().cloned().used().map(|e| e.package_name).collect();
		all.into_iter()
			.filter(move |e| !used.contains(&e.package_name))
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
