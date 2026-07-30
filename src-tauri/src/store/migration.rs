//! One-time (idempotent, re-runnable) sweep that converts legacy `profiles/<device>/<id>.json`
//! files into the new `profiles_v2/<device>/<uuid>/` format. Intended to run once at startup,
//! before anything else touches profile data.

use crate::shared::{Profile, config_dir};
use crate::store::FromAndIntoDiskValue;
use crate::store::profile::manifest::ProfileManifest;
use crate::store::profile::profile::PaginatedProfile;

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use log::{info, warn};
use uuid::Uuid;
use crate::store::profile::DeviceConfig;

/// Fixed, arbitrary namespace used to deterministically derive a profile's new UUID from its
/// legacy `device/id` string. This makes the sweep safe to interrupt and re-run: the same legacy
/// profile always maps to the same UUID, so "does profiles_v2 already have this?" is a plain
/// existence check rather than needing a separate progress log.
const LEGACY_MIGRATION_NAMESPACE: Uuid = Uuid::NAMESPACE_OID;
fn derive_profile_uuid(device: &str, id: &str) -> Uuid {
	Uuid::new_v5(&LEGACY_MIGRATION_NAMESPACE, format!("{device}/{id}").as_bytes())
}

/// Walk every device directory under `profiles/` and migrate any legacy profile that doesn't
/// already have a corresponding entry under `profiles_v2/`. Safe to call on every startup:
/// already-migrated profiles are skipped, and a failure on one profile doesn't block the rest.
pub fn migrate_legacy_profiles() -> Result<()> {
	let profiles_root = config_dir().join("profiles");
	if !profiles_root.exists() {
		// Nothing has ever been saved in the legacy format.
		return Ok(());
	}

	for entry in fs::read_dir(&profiles_root)?.flatten() {
		// Top-level *files* here are DeviceConfig (`<device>.json`, selected-profile tracking),
		// not profile data - only directories hold actual profiles.
		let Ok(file_type) = entry.file_type() else { continue };
		if !file_type.is_dir() {
			continue;
		}

		let device = entry.file_name().to_string_lossy().into_owned();
		let ids = match find_legacy_profile_ids(&entry.path()) {
			Ok(ids) => ids,
			Err(e) => {
				warn!("Failed to scan legacy profiles for device '{device}': {e}");
				continue;
			}
		};

		for id in ids {
			if let Err(e) = migrate_single_profile(&profiles_root, &device, &id) {
				warn!("Failed to migrate legacy profile '{id}' for device '{device}': {e}");
			}
		}
		migrate_device_selection(&profiles_root, &device);
	}

	Ok(())
}

fn migrate_single_profile(profiles_root: &Path, device: &str, id: &str) -> Result<()> {
	let uuid = derive_profile_uuid(device, id);

	if ProfileManifest::exists(device, uuid) {
		// Already converted on a previous run.
		return Ok(());
	}

	let path = legacy_profile_path(profiles_root, device, id);
	let legacy_profile = load_legacy_profile(&path)?;

	let mut paginated = PaginatedProfile::try_from_legacy(device, legacy_profile)?;
	// try_from_legacy mints a random id; force the deterministic one so re-running the sweep
	// (or migrating the same profile twice for any reason) can never produce a duplicate.
	paginated.id = uuid;
	paginated.save()?;

	backup_legacy_file(&path)?;

	info!("Migrated legacy profile '{id}' for device '{device}' -> {uuid}");

	Ok(())
}

/// Carries a device's legacy selected-profile preference forward, if it had one and that
/// profile actually migrated successfully. Safe to call even if there's no legacy config at
/// all - it's a no-op in that case, leaving the "no selection yet" resolution (still TODO) to
/// handle it later.
fn migrate_device_selection(profiles_root: &Path, device: &str) {
	let config_path = profiles_root.join(format!("{device}.json"));

	let Ok(contents) = fs::read(&config_path) else { return };
	let Ok(value) = serde_json::from_slice::<serde_json::Value>(&contents) else {
		warn!("Legacy device config for '{device}' is not valid JSON, leaving unmigrated");
		return;
	};
	let Some(legacy_id) = value.get("selected_profile").and_then(|v| v.as_str()) else { return };

	let uuid = derive_profile_uuid(device, legacy_id);
	if !ProfileManifest::exists(device, uuid) {
		warn!("Selected profile '{legacy_id}' for device '{device}' did not migrate; leaving selection unresolved");
		return;
	}

	if let Err(e) = (DeviceConfig { device: device.to_owned(), selected_profile: uuid }).save() {
		warn!("Failed to persist migrated device selection for '{device}': {e}");
	}
}

/// Read + parse a legacy profile file directly (rather than going through `Store::new`, which
/// silently falls back to a default on any read/parse failure - not what we want here, since we
/// need to be able to tell "genuinely failed" apart from "loaded fine").
fn load_legacy_profile(path: &Path) -> Result<Profile> {
	let contents = fs::read(path)?;
	let value: serde_json::Value = serde_json::from_slice(&contents)?;
	let profile = Profile::from_value(value, path)?;
	Ok(profile)
}

fn legacy_profile_path(profiles_root: &Path, device: &str, id: &str) -> PathBuf {
	let mut path = profiles_root.join(device);
	for part in id.split('/') {
		path = path.join(part);
	}
	path.set_extension("json");
	path
}

/// Move the legacy file out of the way rather than deleting it outright - cheap insurance for
/// the first release this ships in. Once you're confident every install has gone through the
/// sweep, this can be swapped for a straight `fs::remove_file`.
fn backup_legacy_file(path: &Path) -> Result<()> {
	if path.exists() {
		fs::rename(path, path.with_extension("json.migrated"))?;
	}
	// these were only ever fallbacks for `Store`'s own crash-safety; once migrated, they're
	// noise rather than useful backups in their own right.
	let _ = fs::remove_file(path.with_extension("json.bak"));
	let _ = fs::remove_file(path.with_extension("json.temp"));
	Ok(())
}

/// Mirrors `store::profiles::get_device_profiles`'s file-suffix logic, but scoped to a single
/// already-confirmed-existing device directory, with no `create_dir_all` side effect and no
/// "push a fallback 'Default' id if nothing's there" behaviour - both of which would be wrong
/// for a migration pass over what's genuinely on disk.
fn find_legacy_profile_ids(device_dir: &Path) -> Result<Vec<String>> {
	fn strip_known_suffix(name: &str) -> Option<&str> {
		name.strip_suffix(".json").or_else(|| name.strip_suffix(".json.bak")).or_else(|| name.strip_suffix(".json.temp"))
	}

	let mut ids = vec![];

	for entry in fs::read_dir(device_dir)?.flatten() {
		let Ok(file_type) = entry.file_type() else { continue };
		let name = entry.file_name().to_string_lossy().into_owned();

		if file_type.is_file() {
			if let Some(id) = strip_known_suffix(&name) {
				ids.push(id.to_owned());
			}
		} else if file_type.is_dir() {
			// One level of folder nesting, matching the "folder/name" convention ProfileManager
			// already uses.
			for subentry in fs::read_dir(entry.path())?.flatten() {
				let Ok(sub_file_type) = subentry.file_type() else { continue };
				if !sub_file_type.is_file() {
					continue;
				}
				let sub_name = subentry.file_name().to_string_lossy().into_owned();
				if let Some(id) = strip_known_suffix(&sub_name) {
					ids.push(format!("{name}/{id}"));
				}
			}
		}
	}

	// .json / .json.bak / .json.temp for the same profile would otherwise appear as duplicate
	// entries.
	ids.sort();
	ids.dedup();

	Ok(ids)
}
