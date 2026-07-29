//! This is the 'meta' profile, which is used to store basic information about the internals of
//! a profile. It writes as a single file but is designed to be used to build a 'full' profile.

use crate::store::profile::profile::PaginatedProfile;
use crate::store::profile::profile_base_path;
use log::info;
use serde::{Deserialize, Serialize};
use std::fs;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ProfileManifest {
	/// The Device this Profile is associated with
	#[serde(skip)]
	pub(crate) device: String,

	/// The Tracked ID of the Profile
	pub(crate) id: Uuid,

	/// The Name of the Profile
	pub(crate) name: String,

	/// The currently active Page UUID
	pub(crate) current: Uuid,

	/// The UUID of the 'pinned' page
	pub(crate) pinned: Uuid,

	/// List of Pages in the Profile
	pub(crate) pages: Vec<Uuid>,
}

impl ProfileManifest {
	pub fn exists(device: &str, id: Uuid) -> bool {
		let profile_device_path = profile_base_path().join(device).join(id.to_string());
		profile_device_path.exists()
	}

	pub(crate) fn try_from_id(device: &str, id: Uuid) -> anyhow::Result<Self> {
		let profile_device_path = profile_base_path().join(device).join(id.to_string());
		if !profile_device_path.exists() {
			info!("Profile device path does not exist, creating it");
			fs::create_dir_all(&profile_device_path)?;
		}

		let manifest_path = profile_device_path.join("manifest.json");
		let manifest = if !manifest_path.exists() {
			// Create and write a default manifest
			info!("Writing Default Profile Manifest to disk..");

			let current = Uuid::new_v4();
			let default = ProfileManifest {
				device: String::from(device),
				id,
				name: "Default Profile".to_string(),
				current,
				pinned: Uuid::new_v4(),
				pages: vec![current],
			};

			fs::write(manifest_path, &serde_json::to_string_pretty(&default)?)?;

			default
		} else {
			let manifest_content = fs::read_to_string(manifest_path)?;
			serde_json::from_str(&manifest_content)?
		};

		Ok(manifest)
	}

	pub(crate) fn try_from_profile(profile: &PaginatedProfile) -> anyhow::Result<Self> {
		// Load the profile meta
		Ok(Self {
			device: profile.device.clone(),
			id: profile.id,
			name: profile.name.clone(),
			current: profile.pages[profile.current].id,
			pinned: profile.pinned.id,
			pages: profile.pages.iter().map(|p| p.id).collect(),
		})
	}

	pub(crate) fn save(&self) -> anyhow::Result<()> {
		let device = self.device.clone();
		let id = self.id.to_string();
		let profile_device_path = profile_base_path().join(device).join(id);
		if !profile_device_path.exists() {
			fs::create_dir_all(&profile_device_path)?;
		}

		let manifest_path = profile_device_path.join("manifest.json");
		let manifest_content = serde_json::to_string_pretty(&self)?;

		fs::write(manifest_path, &manifest_content)?;

		Ok(())
	}
}
