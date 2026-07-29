use crate::shared::ActionInstance;
use log::warn;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;
use crate::store::profile::profile_base_path;

// So this is theoretically identical to the extisting profile struct
#[derive(Clone, Serialize, Deserialize)]
pub struct Page {
	pub id: Uuid,
	pub keys: Vec<Option<ActionInstance>>,
	pub encoders: Vec<Option<ActionInstance>>,
	pub infobars: Vec<Option<ActionInstance>>,

	#[serde(skip)]
	pub stale: bool,
}

impl Debug for Page {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "Page: {}", self.id)?;
		write!(f, "Stale: {}", self.stale)
	}
}

impl Default for Page {
	fn default() -> Self {
		Self {
			id: Uuid::new_v4(),
			keys: vec![],
			encoders: vec![],
			infobars: vec![],
			stale: false,
		}
	}
}

impl TryFrom<PathBuf> for Page {
	type Error = anyhow::Error;

	fn try_from(path: PathBuf) -> anyhow::Result<Self, Self::Error> {
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
		let default = Page::default();
		fs::write(manifest_path, &serde_json::to_string_pretty(&default)?)?;

		Ok(Page::default())
	}
}

impl Page {
	pub fn save(&self, device: &str, manifest_id: &str) -> anyhow::Result<()> {
		let profile_path = profile_base_path().join(device).join(manifest_id);
		let page_path = profile_path.join("pages").join(self.id.to_string());
		if !page_path.exists() {
			fs::create_dir_all(&page_path)?;
		}
		let page_manifest_path = page_path.join("manifest.json");
		fs::write(page_manifest_path, &serde_json::to_string_pretty(&self)?)?;

		Ok(())
	}
}
