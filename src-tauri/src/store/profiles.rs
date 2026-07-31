use crate::shared::{ActionInstance, DEVICES};
use crate::store::active_profile::{ActiveProfiles, DeviceConfigs};

use std::collections::HashMap;
use std::sync::LazyLock;

use anyhow::{Result, anyhow};
use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Currently Active Profiles
pub static PROFILE_CONFIGS: LazyLock<RwLock<ActiveProfiles>> = LazyLock::new(|| RwLock::new(ActiveProfiles { profiles: HashMap::new() }));

/// Current Device Config
pub static DEVICE_CONFIGS: LazyLock<RwLock<DeviceConfigs>> = LazyLock::new(|| RwLock::new(DeviceConfigs { devices: HashMap::new() }));

pub struct Locks<'a> {
	#[allow(dead_code)]
	pub device_configs: RwLockReadGuard<'a, DeviceConfigs>,
	pub profile_configs: RwLockReadGuard<'a, ActiveProfiles>,
}

pub async fn acquire_locks() -> Locks<'static> {
	let device_configs = DEVICE_CONFIGS.read().await;
	let profile_configs = PROFILE_CONFIGS.read().await;
	Locks { profile_configs, device_configs }
}

pub struct LocksMut<'a> {
	pub profile_configs: RwLockWriteGuard<'a, ActiveProfiles>,
	pub device_configs: RwLockWriteGuard<'a, DeviceConfigs>,
}

pub async fn acquire_locks_mut() -> LocksMut<'static> {
	let device_configs = DEVICE_CONFIGS.write().await;
	let profile_configs = PROFILE_CONFIGS.write().await;
	LocksMut { profile_configs, device_configs }
}

pub async fn get_slot<'a>(context: &crate::shared::Context, locks: &'a Locks<'_>) -> Result<&'a Option<ActionInstance>, anyhow::Error> {
	let device = DEVICES.get(&context.device).ok_or_else(|| anyhow!("device not found"))?;

	let profile = locks.profile_configs.get_profile(&device, context.profile)?;
	let page = &profile.pages[profile.current];

	let configured = match &context.controller[..] {
		"Encoder" => page.encoders.get(context.position as usize).ok_or_else(|| anyhow!("index out of bounds"))?,
		"Infobar" => page.infobars.get(context.position as usize).ok_or_else(|| anyhow!("index out of bounds"))?,
		_ => page.keys.get(context.position as usize).ok_or_else(|| anyhow!("index out of bounds"))?,
	};

	Ok(configured)
}

pub async fn get_slot_mut<'a>(context: &crate::shared::Context, locks: &'a mut LocksMut<'_>) -> Result<&'a mut Option<ActionInstance>, anyhow::Error> {
	let device = DEVICES.get(&context.device).ok_or_else(|| anyhow!("device not found"))?;

	let profile = locks.profile_configs.get_profile_mut(&device, context.profile)?;
	let page = &mut profile.pages[profile.current];

	let configured = match &context.controller[..] {
		"Encoder" => page.encoders.get_mut(context.position as usize).ok_or_else(|| anyhow!("index out of bounds"))?,
		"Infobar" => page.infobars.get_mut(context.position as usize).ok_or_else(|| anyhow!("index out of bounds"))?,
		_ => page.keys.get_mut(context.position as usize).ok_or_else(|| anyhow!("index out of bounds"))?,
	};

	Ok(configured)
}

pub async fn get_instance<'a>(context: &crate::shared::ActionContext, locks: &'a Locks<'_>) -> Result<Option<&'a ActionInstance>, anyhow::Error> {
	let slot = get_slot(&(context.into()), locks).await?;
	if let Some(instance) = slot {
		if instance.context == *context {
			return Ok(Some(instance));
		} else if let Some(children) = &instance.children {
			for child in children {
				if child.context == *context {
					return Ok(Some(child));
				}
			}
		}
	}
	Ok(None)
}

pub async fn get_instance_mut<'a>(context: &crate::shared::ActionContext, locks: &'a mut LocksMut<'_>) -> Result<Option<&'a mut ActionInstance>, anyhow::Error> {
	let slot = get_slot_mut(&(context.into()), locks).await?;
	if let Some(instance) = slot {
		if instance.context == *context {
			return Ok(Some(instance));
		} else if let Some(children) = &mut instance.children {
			for child in children {
				if child.context == *context {
					return Ok(Some(child));
				}
			}
		}
	}
	Ok(None)
}

pub async fn mark_profile_stale(device_id: &str, locks: &mut LocksMut<'_>) -> Result<(), anyhow::Error> {
	// let selected_profile = locks.device_stores.get_selected_profile(device_id)?;
	// let device = DEVICES.get(device_id).ok_or_else(|| anyhow!("device not found"))?;
	// let store = locks.profile_stores.get_profile_store_mut(&device, &selected_profile).await?;
	// store.value.stale = true;

	Ok(())
}

pub async fn save_active_page(device_id: &str, locks: &mut LocksMut<'_>) -> Result<()> {
	let device = DEVICES.get(device_id).ok_or_else(|| anyhow!("device not found"))?;
	let selected_profile = locks.device_configs.get_selected_profile(device_id)?;

	let profile = locks.profile_configs.get_profile_mut(&device, selected_profile)?;
	let page = &mut profile.pages[profile.current];

	page.save()
}

pub async fn flush_stale_profiles() -> Result<(), anyhow::Error> {
	// let mut locks = acquire_locks_mut().await;
	// for store in locks.profile_stores.stores.values_mut() {
	// 	if store.value.stale {
	// 		store.save()?;
	// 		store.value.stale = false;
	// 	}
	// }
	Ok(())
}
