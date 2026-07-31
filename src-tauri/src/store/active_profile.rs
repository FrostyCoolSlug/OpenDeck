use crate::events::registered_plugins;
use crate::shared::{ActionContext, ActionInstance, CATEGORIES, DeviceInfo, ProfileEntry, config_dir, initialise_encoder_layout};
use crate::store::profile::manifest::ProfileManifest;
use crate::store::profile::profile::PaginatedProfile;
use crate::store::profile::{DeviceConfig, get_profile_entries};
use anyhow::{Result, anyhow};
use std::collections::HashMap;
use uuid::Uuid;

pub struct ActiveProfiles {
	pub(crate) profiles: HashMap<String, PaginatedProfile>,
}

impl ActiveProfiles {
	fn identifier(device: &str, id: Uuid) -> String {
		format!("{}-{}", device, id)
	}

	pub fn get_profile(&self, device: &DeviceInfo, id: Uuid) -> Result<&PaginatedProfile> {
		let identifier = Self::identifier(&device.id, id);
		self.profiles.get(&identifier).ok_or_else(|| anyhow!("Profile not Found"))
	}

	pub fn get_profile_mut(&mut self, device: &DeviceInfo, id: Uuid) -> Result<&mut PaginatedProfile> {
		let identifier = Self::identifier(&device.id, id);
		self.profiles.get_mut(&identifier).ok_or_else(|| anyhow!("Profile not Found"))
	}

	pub async fn load_profile(&mut self, device: &DeviceInfo, id: Uuid) -> Result<()> {
		// Firstly, do we already have this profile?
		if self.get_profile(device, id).is_ok() {
			return Ok(());
		}

		// We don't, so load it. Note that this will create a new profile if it doesn't exist.
		let manifest = ProfileManifest::try_from_id(&device.id, id)?;
		let mut profile = PaginatedProfile::try_from_manifest(manifest)?;

		// Firstly, clear any actions which aren't available
		let categories = CATEGORIES.read().await;
		let actions = categories.values().flat_map(|v| v.actions.iter()).collect::<Vec<_>>();

		let registered = registered_plugins().await;
		let plugins_dir = config_dir().join("plugins");

		let keep_instance = |instance: &ActionInstance| {
			if instance.action.plugin == "opendeck" {
				return true;
			}

			let plugin_exists = plugins_dir.join(&instance.action.plugin).exists();
			let plugin_unregistered = !registered.contains(&instance.action.plugin);
			let action_exists = actions.iter().any(|a| a.uuid == instance.action.uuid);

			plugin_exists && (plugin_unregistered || action_exists)
		};

		// Check all the pages of this profile and remove any missing actions
		for page in &mut profile.pages {
			for instance in page.raw_actions_mut() {
				let Some(slot) = instance else {
					continue;
				};

				if !keep_instance(slot) {
					*instance = None;
					continue;
				}

				if let Some(children) = &mut slot.children {
					children.retain(&keep_instance);
				}
			}
		}

		// Now we need to initialise the encoders for all the actions across all pages
		for page in &mut profile.pages {
			for instance in page.encoders.iter_mut().flatten() {
				if instance.action.encoder.is_none() {
					instance.action.encoder = actions.iter().find(|a| a.uuid == *instance.action.uuid).and_then(|a| a.encoder.clone());
				}
				let _ = initialise_encoder_layout(&mut instance.action, None);
			}
		}

		self.profiles.insert(Self::identifier(&device.id, id), profile);
		Ok(())
	}

	pub fn unload_profile(&mut self, device: &DeviceInfo, id: Uuid) {
		let identifier = Self::identifier(&device.id, id);
		self.profiles.remove(&identifier);
	}

	pub async fn create_profile(&mut self, device: &DeviceInfo, name: String) -> Result<ProfileEntry> {
		let profile = PaginatedProfile::new(&device.id, &name);
		profile.save()?;

		let uuid = profile.id;
		let entry = ProfileEntry { id: uuid, name: profile.name.clone() };

		self.load_profile(device, uuid).await?;
		Ok(entry)
	}

	pub fn rename_profile(&mut self, device: &DeviceInfo, id: Uuid, name: &str) -> Result<()> {
		let profile = self.get_profile_mut(device, id)?;
		profile.name = name.to_string();
		profile.save()?;
		Ok(())
	}

	pub fn delete_profile(&mut self, device: &DeviceInfo, id: Uuid) -> Result<()> {
		self.get_profile_mut(device, id)?.delete()?;
		self.unload_profile(device, id);

		Ok(())
	}

	pub fn get_actions_for_plugin(&self, plugin: &str) -> Vec<ActionContext> {
		let mut actions = vec![];

		for profiles in self.profiles.values() {
			for instance in profiles.pages[profiles.current].actions() {
				if instance.action.plugin == plugin {
					actions.push(instance.context.clone());
					continue;
				}

				let Some(children) = &instance.children else {
					continue;
				};

				for child in children {
					if child.action.plugin == plugin {
						actions.push(child.context.clone());
					}
				}
			}
		}

		actions
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
