use crate::shared;
use crate::store::profile::manifest::ProfileManifest;
use crate::store::profile::page::Page;
use crate::store::profile::profile_base_path;
use anyhow::Result;
use log::info;
use std::fs;
use uuid::Uuid;

// This is the 'full' profile object after we've pulled all the parts from disk
#[derive(Debug, Clone)]
pub struct PaginatedProfile {
	/// The Device this Profile is associated with
	pub(crate) device: String,

	/// The Tracked ID of the Profile
	pub(crate) id: Uuid,

	/// The Name of the Profile
	pub(crate) name: String,

	/// The definition of the pinned page
	pub(crate) pinned: Page,

	/// The index of the current active page
	pub(crate) current: usize,

	/// The list of pages in the Profile
	pub(crate) pages: Vec<Page>,
}

impl PaginatedProfile {
	pub(crate) fn try_from_manifest(manifest: ProfileManifest) -> anyhow::Result<Self> {
		// Realistically, we need a cleaner way to do this, device shouldn't need to be passed
		// around here, it's just needed for pathing.

		info!("Loading Profile from disk..");
		let device = manifest.device.clone();
		let profile_path = profile_base_path().join(device).join(manifest.id.to_string());

		// Ok, let's create a profile object, and start assembling it from the parts.
		let mut profile = PaginatedProfile {
			device: manifest.device.clone(),
			id: manifest.id,
			name: manifest.name.clone(),

			pinned: Page::default(),
			current: 0,
			pages: vec![Page::default()],
		};

		let path = profile_path.join("pages");
		if !path.exists() {
			// This should exist, but if it doesn't, we'll just create it.
			fs::create_dir_all(&path)?;
		}

		// First, load out the pinned page
		let pinned = path.join(manifest.pinned.to_string());
		profile.pinned = Page::try_from(pinned)?;

		// Now we need to fill the pages
		for (index, page_id) in manifest.pages.into_iter().enumerate() {
			let page_path = path.join(page_id.to_string());

			// Once the page has loaded, force push the internal ID in case of desync / default
			let mut page = Page::try_from(page_path.clone())?;
			page.id = page_id;

			// Push into the page Vec
			profile.pages.push(page);

			if page_id == manifest.current {
				profile.current = index;
			}
		}
		Ok(profile)
	}

	pub fn try_from_legacy(device: &str, value: shared::Profile) -> anyhow::Result<Self> {
		Ok(Self {
			device: device.to_string(),
			id: Uuid::new_v4(),
			name: value.id,
			pinned: Page {
				id: Uuid::new_v4(),
				keys: vec![],
				encoders: vec![],
				infobars: vec![],

				stale: true,
			},
			current: 0,
			pages: vec![Page {
				id: Uuid::new_v4(),
				keys: value.keys,
				encoders: value.sliders,
				infobars: value.infobars,
				stale: true,
			}],
		})
	}

	pub fn save(&self) -> Result<()> {
		// Firstly, Create and Save the main Manifest
		let manifest = ProfileManifest::try_from_profile(self)?;
		manifest.save()?;

		let manifest_device = manifest.device;
		let manifest_id = manifest.id.to_string();

		// Next, we need to save the pinned page
		self.pinned.save(&manifest_device, &manifest_id)?;

		// Then, the rest of the pages
		for page in self.pages.iter() {
			page.save(&manifest_device, &manifest_id)?;
		}

		Ok(())
	}

	pub fn add_page(&mut self) {
		self.pages.push(Page {
			id: Uuid::new_v4(),
			keys: vec![],
			encoders: vec![],
			infobars: vec![],
			stale: false,
		});
	}

	pub fn remove_page(&mut self) {
		// We don't just need to remove this page from the struct, but we also need
		// to kill its filesystem representation.

		// TODO: Work out how the official app handles this.
	}

	pub fn get_current_page(&self) -> &Page {
		&self.pages[self.current]
	}

	pub fn set_current_page(&mut self, index: usize) -> &Page {
		// What to do..
		// 1. If the page is already the current page, do nothing.
		// 2. If the current page is stale, we need to save it to disk.
		// 3. Switch the Page
		// 4. Return the new page

		self.current = index;
		&self.pages[self.current]
	}
}
