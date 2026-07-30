use crate::shared::{DeviceInfo, config_dir};
use crate::store::profile::profile::PaginatedProfile;
use crate::store::profile::{DeviceConfig, get_profile_entries, profile};
use anyhow::{Result, anyhow};
use std::collections::HashMap;
use uuid::Uuid;
use crate::store::profile::manifest::ProfileManifest;

pub struct ActiveProfiles {
	pub(crate) profiles: HashMap<String, PaginatedProfile>,
}

impl ActiveProfiles {
	fn identifier(device: &str, id: Uuid) -> String {
		format!("{}-{}", device, id)
	}

	pub fn get_active_profile(&self, device: &DeviceInfo, id: Uuid) -> Result<&PaginatedProfile> {
		let identifier = Self::identifier(&device.id, id);
		self.profiles.get(&identifier).ok_or_else(|| anyhow!("Profile not Found"))
	}
	pub fn get_active_profile_mut(&mut self, device: &DeviceInfo, id: Uuid) -> Result<&mut PaginatedProfile> {
		let identifier = Self::identifier(&device.id, id);
		self.profiles.get_mut(&identifier).ok_or_else(|| anyhow!("Profile not Found"))
	}

	pub fn load_profile(&mut self, device: &DeviceInfo, id: Uuid) -> Result<&PaginatedProfile> {
		// Firstly, do we already have this profile?
		if self.get_active_profile(device, id).is_ok() {
			return self.get_active_profile(device, id);
		}

		// We don't, so load it. Note that this will create a new profile if it doesn't exist.
		let manifest = ProfileManifest::try_from_id(&device.id, id)?;
		let profile = PaginatedProfile::try_from_manifest(manifest)?;

		// Store it
		self.profiles.insert(Self::identifier(&device.id, id), profile);

		// Sent it back
		Ok(self.get_active_profile(device, id)?)
	}
}

pub struct DeviceConfigs {
	pub(crate) devices: HashMap<String, DeviceConfig>,
}

impl DeviceConfigs {
	pub fn get_selected_profile(&mut self, device: &str) -> Result<Uuid> {
		// Load or create a config if it doesn't exist
		if !self.devices.contains_key(device) {
			if let Ok(config) = DeviceConfig::try_from_device(device) {
				self.devices.insert(device.to_owned(), config);
			} else {
				// Create a new config, associated profile will be handled later
				let device_config = DeviceConfig {
					device: device.to_string(),
					selected_profile: Uuid::new_v4(),
				};

				device_config.save()?;
				self.devices.insert(device.to_owned(), DeviceConfig::default());
			}
		}

		let profiles = get_profile_entries(device);
		let config_id = self.devices.get(device).unwrap().selected_profile.clone();

		// Is this profile in our list?
		if profiles.iter().any(|p| p.id == config_id) {
			return Ok(config_id);
		}

		// Either grab the first ID, or create a new profile
		let id = match profiles.first() {
			Some(profile) => profile.id.clone(),
			None => {
				// There are no available profiles for this device, create a default one
				let config = self.devices.get_mut(device).unwrap();
				let profile = PaginatedProfile::new(device, "Default");

				config.selected_profile = profile.id.clone();

				// Save the new config, and the new profile to disk
				config.save()?;
				profile.save()?;

				profile.id
			}
		};

		Ok(id)
	}

	pub fn set_selected_profile(&mut self, device: &str, id: Uuid) -> Result<()> {
		if let Some(device_config) = self.devices.get_mut(device) {
			device_config.selected_profile = id;
			device_config.save()?;
		} else {
			let default = DeviceConfig {
				device: device.to_string(),
				selected_profile: id,
			};
			default.save()?;
			self.devices.insert(device.to_owned(), default);
		}
		Ok(())
	}
}
