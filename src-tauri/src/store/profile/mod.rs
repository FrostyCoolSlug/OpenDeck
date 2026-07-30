use crate::shared::{config_dir, ProfileEntry};
use crate::store::profile::manifest::ProfileManifest;
use crate::store::profile::profile::PaginatedProfile;
use anyhow::{Result, bail};
use log::warn;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

pub mod manifest;
pub mod page;
pub mod profile;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceConfig {
	#[serde(skip)]
	pub device: String,
	pub selected_profile: Uuid,
}

impl Default for DeviceConfig {
	fn default() -> Self {
		Self {
			device: "".to_owned(),
			selected_profile: Uuid::new_v4(),
		}
	}
}

impl DeviceConfig {
	fn path(device: &str) -> PathBuf {
		profile_base_path().join(format!("{device}.json"))
	}

	pub fn exists(device: &str) -> bool {
		Self::path(device).exists()
	}

	pub fn try_from_device(device: &str) -> Result<Self> {
		let content = fs::read_to_string(Self::path(device))?;
		let mut selected: DeviceConfig = serde_json::from_str(&content)?;
		selected.device = device.to_owned();
		Ok(selected)
	}

	pub fn save(&self) -> Result<()> {
		let path = Self::path(&self.device);
		if let Some(parent) = path.parent() {
			fs::create_dir_all(parent)?;
		}
		fs::write(path, serde_json::to_string_pretty(&self)?)?;
		Ok(())
	}
}

pub fn load_profile(device: &str, id: Uuid) -> Result<PaginatedProfile> {
	let manifests = get_profile_manifests(device);
	for manifest in manifests {
		if manifest.id == id {
			match PaginatedProfile::try_from_manifest(manifest) {
				Ok(profile) => return Ok(profile),
				Err(e) => bail!("Failed to load Profile: {}", e),
			}
		}
	}

	bail!("Profile ID not found: {}", id);
}

pub fn get_selected_profile(device: &str) -> Result<Uuid> {
	if let Ok(selected) = DeviceConfig::try_from_device(device)
		&& ProfileManifest::exists(device, selected.selected_profile)
	{
		return Ok(selected.selected_profile);
	}

	// Nothing valid selected yet - fall back to whatever's first, if anything exists.
	if let Some(first) = get_profile_manifests(device).into_iter().next() {
		DeviceConfig {
			device: device.to_owned(),
			selected_profile: first.id,
		}
		.save()?;
		return Ok(first.id);
	}

	// Nothing exists at all for this device - create a fresh default and select it.
	// try_from_id already handles "doesn't exist yet -> write a default manifest".
	let manifest = ProfileManifest::try_from_id(device, Uuid::new_v4())?;
	DeviceConfig {
		device: device.to_owned(),
		selected_profile: manifest.id,
	}
	.save()?;
	Ok(manifest.id)
}

pub fn get_profile_entries(device: &str) -> Vec<ProfileEntry> {
	let mut names = vec![];

	let manifests = get_profile_manifests(device);
	for manifest in manifests {
		names.push(ProfileEntry { id: manifest.id, name: manifest.name });
	}

	names
}

fn get_profile_manifests(device: &str) -> Vec<ProfileManifest> {
	let mut manifests = vec![];

	// Get this profiles base path
	let profile_base_path = profile_base_path().join(device);
	if !profile_base_path.exists() {
		return vec![];
	}

	// Find all directories in this path
	if let Ok(entries) = fs::read_dir(&profile_base_path) {
		for entry in entries.flatten() {
			let Ok(file_type) = entry.file_type() else {
				continue;
			};

			if !file_type.is_dir() {
				continue;
			}

			let Some(id) = entry.file_name().to_str().and_then(|s| Uuid::parse_str(s).ok()) else {
				warn!("Path in Profile is not a valid UUID: {:?}", entry.file_name());
				continue;
			};

			match ProfileManifest::try_from_id(device, id) {
				Ok(manifest) => manifests.push(manifest),
				Err(e) => warn!("Failed to load Profile Manifest: {}", e),
			}
		}
	}

	manifests
}

pub fn profile_base_path() -> PathBuf {
	config_dir().join("profiles_v2")
}
