use super::Error;
use log::debug;

use crate::shared::{DEVICES, ProfileEntry, ProfileView};
use crate::store::profiles::acquire_locks_mut;

use crate::events::outbound::{devices, will_appear};
use crate::store::profile::get_profile_entries;
use tauri::{AppHandle, Emitter, Manager, command};
use uuid::Uuid;

#[command]
pub fn get_profiles(device: &str) -> Vec<ProfileEntry> {
	get_profile_entries(device)
}

#[command]
pub async fn get_selected_profile(device: String) -> Result<ProfileView, Error> {
	debug!("Getting Active Profile for: {}", device);
	let Some(device_info) = DEVICES.get(&device) else {
		return Err(Error::new(format!("device {device} not found")));
	};

	let mut locks = acquire_locks_mut().await;
	let selected = locks.device_configs.get_selected_profile(&device)?;

	let profile = locks.active_profiles.get_profile(&device_info, selected)?;

	let profile_view = ProfileView::from(profile);
	debug!("{:#?}", profile_view);

	Ok(profile_view)
}

#[command]
pub async fn set_selected_profile(device: String, id: Uuid) -> Result<(), Error> {
	debug!("Setting Active Profile for: {} to {}", device, id);

	let Some(device_info) = DEVICES.get(&device) else {
		debug!("Device {} not found", device);
		return Err(Error::new(format!("device {device} not found")));
	};

	// Firstly, we should get the full current profile
	let mut locks = acquire_locks_mut().await;
	let current_id = locks.device_configs.get_selected_profile(&device)?;

	if current_id == id {
		// Do nothing here, nothing is changing, and we shouldn't trigger a will_appear on something
		// without first telling it to disappear.
		return Ok(());
	}

	// Get the full current profile, and tell its actions they are going to disappear
	let current_profile = locks.active_profiles.get_profile(&device_info, current_id)?;
	let current_page = &current_profile.pages[current_profile.current];
	current_page.save()?;

	// We need to itearte over all the page actions, and tell them to disappear
	for instance in current_page.actions() {
		// So multiaction and toggleaction are special, they have children and those need to go
		if !matches!(instance.action.uuid.as_str(), "opendeck.multiaction" | "opendeck.toggleaction") {
			let _ = will_appear::will_disappear(instance, false).await;
		} else {
			for child in instance.children.as_ref().unwrap() {
				let _ = will_appear::will_disappear(child, false).await;
			}
		}
	}

	// Send out a message to clear everything from this screen
	let _ = devices::clear_screen(device.clone()).await?;

	// Next, grab the profile we're about to change to
	let new_profile = locks.active_profiles.get_profile(&device_info, id)?;
	let new_page = &new_profile.pages[new_profile.current];

	// Let all the actions know they're going to appear
	for instance in new_page.actions() {
		if !matches!(instance.action.uuid.as_str(), "opendeck.multiaction" | "opendeck.toggleaction") {
			let _ = will_appear::will_appear(instance).await;
		} else {
			for child in instance.children.as_ref().unwrap() {
				let _ = will_appear::will_appear(child).await;
			}
		}
	}

	// Finally, commit the profile change
	let _ = locks.device_configs.set_selected_profile(&device, id)?;

	Ok(())
}

#[command]
pub async fn create_profile(device: String, name: String) -> Result<ProfileEntry, Error> {
	let Some(device_info) = DEVICES.get(&device) else {
		return Err(Error::new(format!("device {device} not found")));
	};

	let mut locks = acquire_locks_mut().await;
	let entry = locks.active_profiles.create_profile(&device_info, name).await?;

	Ok(entry)
}

#[command]
pub async fn delete_profile(device: String, profile: Uuid) -> Result<(), Error> {
	let Some(device_info) = DEVICES.get(&device) else {
		return Err(Error::new(format!("device {device} not found")));
	};

	let mut locks = acquire_locks_mut().await;
	let current_id = locks.device_configs.get_selected_profile(&device)?;

	if profile == current_id {
		return Err(Error::new("Cannot delete active profile".to_string()));
	}

	// Nuke it
	locks.active_profiles.delete_profile(&device_info, profile)?;
	Ok(())
}

#[command]
pub async fn rename_profile(device: String, id: Uuid, name: String) -> Result<(), Error> {
	let Some(device_info) = DEVICES.get(&device) else {
		return Err(Error::new(format!("device {device} not found")));
	};

	let mut locks = acquire_locks_mut().await;
	locks.active_profiles.rename_profile(&device_info, id, &name)?;

	Ok(())
}

pub async fn rerender_images(app: &AppHandle) -> Result<(), anyhow::Error> {
	let window = app.get_webview_window("main").unwrap();
	window.emit("rerender_images", ())?;
	Ok(())
}
