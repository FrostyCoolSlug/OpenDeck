use crate::shared::{Action, ActionContext, ActionInstance, ActionState};
use crate::store::profile::profile_base_path;
use anyhow::Result;
use log::{debug, warn};
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
		debug!("Loading Page {} of Profile {} for {}", page_id, profile_id, device);

		let path = Self::build_page_path(device, profile_id, page_id);

		if !path.exists() {
			fs::create_dir_all(&path)?;
		}

		let manifest_path = path.join("manifest.json");
		if manifest_path.exists() {
			let manifest_file = fs::read_to_string(manifest_path)?;
			let page: PageStorage = serde_json::from_str(&manifest_file)?;
			return Ok(page.into_page(device, profile_id));
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

		let disk: PageStorage = self.into();
		let page_manifest_path = page_path.join("manifest.json");
		fs::write(page_manifest_path, &serde_json::to_string_pretty(&disk)?)?;

		Ok(())
	}

	pub(crate) fn delete(&self) -> Result<()> {
		let page_path = self.get_page_path();
		fs::remove_dir_all(page_path)?;

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

#[derive(Serialize, Deserialize)]
pub struct PageStorage {
	pub id: Uuid,

	pub keys: Vec<Option<ActionInstanceStorage>>,
	pub encoders: Vec<Option<ActionInstanceStorage>>,
	pub infobars: Vec<Option<ActionInstanceStorage>>,
}

impl From<&Page> for PageStorage {
	fn from(value: &Page) -> Self {
		Self {
			id: value.id,

			keys: value.keys.clone().into_iter().map(|x| x.map(Into::into)).collect(),
			encoders: value.encoders.clone().into_iter().map(|x| x.map(Into::into)).collect(),
			infobars: value.infobars.clone().into_iter().map(|x| x.map(Into::into)).collect(),
		}
	}
}

impl PageStorage {
	fn into_page(self, device: &str, profile_id: Uuid) -> Page {
		Page {
			id: self.id,
			keys: self.keys.clone().into_iter().map(|x| x.map(|v| v.into_action_instance(device, profile_id))).collect(),
			encoders: self.encoders.clone().into_iter().map(|x| x.map(|v| v.into_action_instance(device, profile_id))).collect(),
			infobars: self.infobars.clone().into_iter().map(|x| x.map(|v| v.into_action_instance(device, profile_id))).collect(),
			profile_id,
			profile_device: device.to_string(),
			stale: false,
		}
	}
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ActionInstanceStorage {
	/// An instance of an action.
	pub action: Action,
	pub context: ActionContextStorage,
	pub states: Vec<ActionState>,
	pub current_state: u16,
	pub settings: serde_json::Value,
	pub children: Option<Vec<ActionInstanceStorage>>,
}

impl From<ActionInstance> for ActionInstanceStorage {
	fn from(value: ActionInstance) -> Self {
		ActionInstanceStorage {
			action: value.action,
			context: ActionContextStorage::from(value.context),
			states: value.states,
			current_state: value.current_state,
			settings: value.settings,
			children: value.children.map(|c| c.into_iter().map(|v| v.into()).collect()),
		}
	}
}

impl ActionInstanceStorage {
	fn into_action_instance(self, device: &str, profile_id: Uuid) -> ActionInstance {
		ActionInstance {
			action: self.action.clone(),
			context: self.context.into_action_context(device, profile_id),
			states: self.states.clone(),
			current_state: self.current_state.clone(),
			settings: self.settings.clone(),
			children: self.children.clone().map(|c| c.into_iter().map(|v| v.into_action_instance(device, profile_id)).collect()),
		}
	}
}

#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ActionContextStorage {
	pub controller: String,
	pub position: u8,
	pub index: u16,
}

impl From<ActionContext> for ActionContextStorage {
	fn from(value: ActionContext) -> Self {
		Self {
			controller: value.controller,
			position: value.position,
			index: value.index,
		}
	}
}

impl ActionContextStorage {
	fn into_action_context(self, device: &str, profile_id: Uuid) -> ActionContext {
		ActionContext {
			device: device.to_string(),
			profile: profile_id,

			controller: self.controller.clone(),
			position: self.position.clone(),
			index: self.index.clone(),
		}
	}
}
