use crate::shared::config_dir;
use crate::store::profile::manifest::ProfileManifest;
use crate::store::profile::profile::PaginatedProfile;
use anyhow::{Result, bail};
use log::warn;
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

pub mod manifest;
pub mod page;
pub mod profile;

pub struct ProfileEntry {
	pub id: Uuid,
	pub name: String,
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

pub fn get_profile_names(device: &str) -> Vec<ProfileEntry> {
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
