use super::PayloadEvent;
use anyhow::{Result, bail};

use crate::plugins::DEVICE_NAMESPACES;
use crate::shared::DEVICES;
use crate::store::profiles::{acquire_locks_mut, get_device_profiles};

use crate::device_sleep;
use crate::events::frontend;
use crate::events::outbound::{devices, will_appear};
use crate::store::profile::get_profile_entries;
use serde::Deserialize;

pub async fn register_device(uuid: &str, mut event: PayloadEvent<crate::shared::DeviceInfo>) -> Result<()> {
	let namespaces = DEVICE_NAMESPACES.read().await;
	let namespace = namespaces.get(&event.payload.id[..2]).map(String::as_str);
	if !uuid.is_empty() && Some(uuid) != namespace {
		bail!("plugin {uuid} is not registered for device namespace {}", &event.payload.id[..2]);
	}

	// Grab a write lock
	let mut locks = acquire_locks_mut().await;

	// Load up the known Profiles for this Device
	for profile in get_profile_entries(&event.payload.id) {
		locks.active_profiles.load_profile(&event.payload, profile.id)?;
	}

	// Store the plugin UUID for this device
	event.payload.plugin = uuid.to_owned();

	// Add this device to our devices store, and send initial connection messages
	let _ = devices::device_did_connect(&event.payload.id, (&event.payload).into()).await;
	DEVICES.insert(event.payload.id.clone(), event.payload.clone());

	let _ = device_sleep::apply_initial_device_sleep(&event.payload.id).await;
	frontend::update_devices().await;

	// Ok, lets grab the selected profile
	let selected_profile = locks.device_configs.get_selected_profile(&event.payload.id)?;
	let profile = locks.active_profiles.get_profile(&event.payload, selected_profile)?;

	// Let all the actions know they're about to go live
	for action in profile.pages[profile.current].actions() {
		let _ = will_appear::will_appear(action).await;
	}

	use tauri_plugin_aptabase::EventTracker;
	let tracker = crate::APP_HANDLE.get().unwrap();
	let _ = tracker.track_event("device_registered", Some(serde_json::json!({ "name": event.payload.name })));

	Ok(())
}

pub async fn deregister_device(uuid: &str, event: PayloadEvent<String>) -> Result<()> {
	let namespaces = DEVICE_NAMESPACES.read().await;
	let namespace = namespaces.get(&event.payload[..2]).map(|x| x.as_str());
	if !uuid.is_empty() && Some(uuid) != namespace {
		bail!("plugin {uuid} is not registered for device namespace {}", &event.payload[..2]);
	}

	let Some(device_info) = DEVICES.get(&event.payload) else { return Ok(()) };
	let mut locks = acquire_locks_mut().await;

	// Firstly, we need to grab the current profile, and tell everything it's going away
	let selected_profile = locks.device_configs.get_selected_profile(&device_info.id)?;
	let profile = locks.active_profiles.get_profile(&device_info, selected_profile)?;

	// Let all the actions know they're disappearing
	for action in profile.pages[profile.current].actions() {
		let _ = will_appear::will_disappear(action, false).await;
	}

	// Finally, unload all the profiles for this device
	for profile in get_profile_entries(&device_info.id) {
		locks.active_profiles.unload_profile(&device_info, profile.id);
	}

	drop(locks);

	// Tell the world it's gone and remove it
	let _ = devices::device_did_disconnect(&event.payload).await;
	DEVICES.remove(&event.payload);

	// Wait a moment, then update the frontend
	device_sleep::deregister_device(&event.payload);
	frontend::update_devices().await;

	Ok(())
}

#[derive(Deserialize)]
pub struct PressPayload {
	pub device: String,
	pub position: u8,
}

pub async fn key_down(event: PayloadEvent<PressPayload>) -> Result<(), anyhow::Error> {
	if device_sleep::note_activity(&event.payload.device).await.unwrap_or(false) {
		return Ok(());
	}
	crate::events::outbound::keypad::key_down(&event.payload.device, event.payload.position).await
}

pub async fn key_up(event: PayloadEvent<PressPayload>) -> Result<(), anyhow::Error> {
	if device_sleep::note_activity(&event.payload.device).await.unwrap_or(false) {
		return Ok(());
	}
	crate::events::outbound::keypad::key_up(&event.payload.device, event.payload.position).await
}

#[derive(Deserialize)]
pub struct TicksPayload {
	pub device: String,
	pub position: u8,
	pub ticks: i16,
}

pub async fn encoder_change(event: PayloadEvent<TicksPayload>) -> Result<(), anyhow::Error> {
	if device_sleep::note_activity(&event.payload.device).await.unwrap_or(false) {
		return Ok(());
	}
	crate::events::outbound::encoder::dial_rotate(&event.payload.device, event.payload.position, event.payload.ticks).await
}

pub async fn encoder_down(event: PayloadEvent<PressPayload>) -> Result<(), anyhow::Error> {
	if device_sleep::note_activity(&event.payload.device).await.unwrap_or(false) {
		return Ok(());
	}
	crate::events::outbound::encoder::dial_press(&event.payload.device, "dialDown", event.payload.position).await
}

pub async fn encoder_up(event: PayloadEvent<PressPayload>) -> Result<(), anyhow::Error> {
	if device_sleep::note_activity(&event.payload.device).await.unwrap_or(false) {
		return Ok(());
	}
	crate::events::outbound::encoder::dial_press(&event.payload.device, "dialUp", event.payload.position).await
}

#[derive(Deserialize)]
pub struct TouchscreenPressPayload {
	pub device: String,
	pub position: u8,
	pub x: u16,
	pub y: u16,
	#[serde(default)]
	pub hold: bool,
}

pub async fn touchscreen_press(event: PayloadEvent<TouchscreenPressPayload>) -> Result<(), anyhow::Error> {
	if crate::device_sleep::note_activity(&event.payload.device).await.unwrap_or(false) {
		return Ok(());
	}
	crate::events::outbound::encoder::touch_tap(&event.payload.device, event.payload.position, event.payload.x, event.payload.y, event.payload.hold).await
}

pub async fn rerender_images(_event: PayloadEvent<String>) -> Result<(), anyhow::Error> {
	crate::events::frontend::profiles::rerender_images(crate::APP_HANDLE.get().unwrap()).await?;
	Ok(())
}
