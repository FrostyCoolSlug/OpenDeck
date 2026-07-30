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
	/// Creates a new profile with a single page attached
	pub(crate) fn new(device: &str, name: &str) -> Self {
		let profile_id = Uuid::new_v4();

		Self {
			device: device.to_string(),
			id: profile_id,
			name: name.to_string(),
			pinned: Page::default_with(device, profile_id),
			current: 0,
			pages: vec![Page::default_with(device, profile_id)],
		}
	}

	pub(crate) fn try_from_manifest(manifest: ProfileManifest) -> anyhow::Result<Self> {
		// Realistically, we need a cleaner way to do this, device shouldn't need to be passed
		// around here, it's just needed for pathing.

		info!("Loading Profile from disk..");
		let device = manifest.device.clone();
		let profile_path = profile_base_path().join(&device).join(manifest.id.to_string());

		// Ok, let's create a profile object, and start assembling it from the parts.
		let mut profile = PaginatedProfile {
			device: manifest.device.clone(),
			id: manifest.id,
			name: manifest.name.clone(),

			pinned: Page::default_with(&device, manifest.id),
			current: 0,
			pages: vec![],
		};

		let path = profile_path.join("pages");
		if !path.exists() {
			// This should exist, but if it doesn't, we'll just create it.
			fs::create_dir_all(&path)?;
		}

		// First, load out the pinned page
		let pinned = path.join(manifest.pinned.to_string());
		profile.pinned = Page::load_from(&device, manifest.id, manifest.pinned)?;

		// Now we need to fill the pages
		for (index, page_id) in manifest.pages.into_iter().enumerate() {
			// Once the page has loaded, force push the internal ID in case of desync / default
			let mut page = Page::load_from(&device, manifest.id, page_id)?;
			page.id = page_id;

			// Push into the page Vec
			profile.pages.push(page);

			if page_id == manifest.current {
				profile.current = index;
			}
		}

		// If there are no pages loaded, force one into the profile
		if profile.pages.is_empty() {
			profile.add_page();
		}

		Ok(profile)
	}

	pub fn try_from_legacy(device: &str, value: shared::Profile) -> anyhow::Result<Self> {
		let profile_id = Uuid::new_v4();

		Ok(Self {
			device: device.to_string(),
			id: profile_id,
			name: value.id,
			pinned: Page {
				id: Uuid::new_v4(),
				keys: vec![],
				encoders: vec![],
				infobars: vec![],

				profile_device: device.to_string(),
				profile_id,
				stale: true,
			},
			current: 0,
			pages: vec![Page {
				id: profile_id,
				keys: value.keys,
				encoders: value.sliders,
				infobars: value.infobars,

				profile_device: device.to_string(),
				profile_id,
				stale: true,
			}],
		})
	}

	pub fn save(&self) -> Result<()> {
		// Firstly, Create and Save the main Manifest
		let manifest = ProfileManifest::try_from_profile(self)?;
		manifest.save()?;

		// Next, we need to save the pinned page
		self.pinned.save()?;

		// Then, the rest of the pages
		for page in self.pages.iter() {
			page.save()?;
		}

		Ok(())
	}

	pub fn add_page(&mut self) {
		let page = Page {
			id: Uuid::new_v4(),
			keys: vec![],
			encoders: vec![],
			infobars: vec![],

			profile_device: self.device.clone(),
			profile_id: self.id,
			stale: false,
		};

		// Save and push
		let _ = page.save();
		self.pages.push(page);
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
