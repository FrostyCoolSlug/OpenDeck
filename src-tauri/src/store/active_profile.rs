use crate::shared::DeviceInfo;
use crate::store::Store;
use crate::store::profile::{profile, DeviceConfig};
use crate::store::profile::profile::PaginatedProfile;
use anyhow::anyhow;
use std::collections::HashMap;
use std::path::PathBuf;

pub struct ActiveProfiles {
	pub(crate) profiles: HashMap<String, PaginatedProfile>,
}

pub struct DeviceConfigs {
	pub(crate) devices: HashMap<String, DeviceConfig>
}

impl ActiveProfiles {
	fn canonical_id(device: &str, id: &str) -> String {
		// TODO: This probably needs to be more robust..

		if cfg!(target_os = "windows") {
			PathBuf::from(device).join(id.replace('/', "\\")).to_str().unwrap().to_owned()
		} else {
			PathBuf::from(device).join(id).to_str().unwrap().to_owned()
		}
	}

	pub fn get_active_profile(&self, device: &DeviceInfo, id: &str) -> Result<&PaginatedProfile, anyhow::Error> {
		self.profiles.get(&Self::canonical_id(&device.id, id)).ok_or_else(|| anyhow!("profile not found"))
	}
}
