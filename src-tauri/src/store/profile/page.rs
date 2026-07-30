use crate::shared::ActionInstance;
use crate::store::profile::profile_base_path;
use anyhow::Result;
use log::warn;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

// So this is theoretically identical to the extisting profile struct
#[derive(Clone, Serialize, Deserialize)]
pub struct Page {
	pub id: Uuid,
	pub keys: Vec<Option<ActionInstance>>,
	pub encoders: Vec<Option<ActionInstance>>,
	pub infobars: Vec<Option<ActionInstance>>,

	// Profile ID is transient and is used when loading and saving the page to
	// make sure it can go in the correct place without awkward function calls
	#[serde(skip)]
	pub profile_id: Uuid,

	// As above, the device this page is associated with, again for use when
	// saving.
	#[serde(skip)]
	pub profile_device: String,

	// Is Stale defines whether something has been changed in this page and
	// needs to be saved.
	#[serde(skip)]
	pub stale: bool,
}

impl Debug for Page {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "Page: {}", self.id)?;
		write!(f, "Stale: {}", self.stale)
	}
}

impl Page {
	pub(crate) fn default_with(profile_device: &str, profile_id: Uuid) -> Self {
		Page {
			id: Uuid::new_v4(),
			keys: vec![],
			encoders: vec![],
			infobars: vec![],

			profile_id,
			profile_device: profile_device.to_string(),
			stale: false,
		}
	}

	pub(crate) fn load_from(device: &str, profile_id: Uuid, page_id: Uuid) -> Result<Self> {
		let path = Self::build_page_path(device, profile_id, page_id);

		if !path.exists() {
			fs::create_dir_all(&path)?;
		}

		let manifest_path = path.join("manifest.json");
		if manifest_path.exists() {
			let manifest_file = fs::read_to_string(manifest_path)?;
			let page: Page = serde_json::from_str(&manifest_file)?;

			return Ok(page);
		}

		warn!("Page Path does not exist, creating blank page");
		let default = Page::default_with(device, profile_id);

		fs::write(manifest_path, &serde_json::to_string_pretty(&default)?)?;
		Ok(default)
	}

	pub(crate) fn save(&self) -> Result<()> {
		let page_path = self.get_page_path();

		if !page_path.exists() {
			fs::create_dir_all(&page_path)?;
		}
		let page_manifest_path = page_path.join("manifest.json");
		fs::write(page_manifest_path, &serde_json::to_string_pretty(&self)?)?;

		Ok(())
	}

	pub fn actions(&self) -> impl Iterator<Item = &ActionInstance> {
		self.keys.iter().chain(&self.encoders).chain(&self.infobars).flatten()
	}

	fn get_page_path(&self) -> PathBuf {
		profile_base_path()
			.join(self.profile_device.clone())
			.join(self.profile_id.to_string())
			.join("pages")
			.join(self.id.to_string())
	}

	fn build_page_path(device: &str, profile_id: Uuid, page_id: Uuid) -> PathBuf {
		profile_base_path().join(device.to_string()).join(profile_id.to_string()).join("pages").join(page_id.to_string())
	}
}
