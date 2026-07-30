<script lang="ts">
	import type { DeviceInfo } from "$lib/DeviceInfo";
	import type { ProfileView } from "$lib/ProfileView";
	import type { ProfileEntry } from "$lib/ProfileEntry";

	import Browsers from "phosphor-svelte/lib/Browsers";
	import Copy from "phosphor-svelte/lib/Copy";
	import FloppyDisk from "phosphor-svelte/lib/FloppyDisk";
	import Pencil from "phosphor-svelte/lib/Pencil";
	import Trash from "phosphor-svelte/lib/Trash";
	import Popup from "./Popup.svelte";

	import { t } from "$lib/i18n";
	import { inspectedInstance } from "$lib/propertyInspector";

	import { invoke } from "@tauri-apps/api/core";
	import { listen } from "@tauri-apps/api/event";
	import { message } from "@tauri-apps/plugin-dialog";

	let folders: { [name: string]: ProfileEntry[] } = {};
	let value: string;
	async function getProfiles(device: DeviceInfo) {
		let profiles: ProfileEntry[] = await invoke("get_profiles", { device: device.id });
		folders = {};
		for (const entry of profiles) {
			let folder = entry.name.includes("/") ? entry.name.split("/")[0] : "";
			if (folders[folder]) folders[folder].push(entry);
			else folders[folder] = [entry];
		}
		profile = await invoke("get_selected_profile", { device: device.id });
		value = profile.id;
		oldValue = value;
	}

	export let device: DeviceInfo;
	getProfiles(device);

	export let profile: ProfileView;
	export async function setProfile(id: string) {
		if (!device || !id) return;
		if (value != id) {
			value = id;
			return;
		}
		await invoke("set_selected_profile", { device: device.id, id });
		profile = await invoke("get_selected_profile", { device: device.id });

		$inspectedInstance = null;
	}

	listen("rerender_images", async () => {
		try {
			profile = await invoke("get_selected_profile", { device: device.id });
		} catch {}
	});

	async function deleteProfile(id: string) {
		for (const devices of Object.values(applicationProfiles)) {
			if (devices[device.id] == id) {
				delete devices[device.id];
				applicationProfiles = applicationProfiles;
			}
		}
		await invoke("delete_profile", { device: device.id, profile: id });
		for (const folder in folders) {
			const index = folders[folder].findIndex((entry) => entry.id == id);
			if (index != -1) folders[folder].splice(index, 1);
		}
		folders = folders;
	}

	let renamingProfile: string | null = null;
	let renameInput: HTMLInputElement;
	let newName: string = "";

	async function saveRenamedProfile(id: string) {
		if (!renameInput.checkValidity() || !newName) return;

		let entry: ProfileEntry | undefined;
		let oldFolder = "";
		for (const folder in folders) {
			const found = folders[folder].find((e) => e.id == id);
			if (found) {
				entry = found;
				oldFolder = folder;
				break;
			}
		}
		if (!entry) return;

		if (newName == entry.name) {
			renamingProfile = null;
			return;
		}

		// Check if a profile with the new name already exists
		const allProfiles = Object.values(folders).flat();
		if (allProfiles.some((e) => e.name == newName)) {
			message($t("profile_manager.rename.exists", { id: newName }), { title: $t("profile_manager.rename.failed"), buttons: { ok: $t("dialog.ok") } });
			return;
		}

		try {
			await invoke("rename_profile", { device: device.id, id, name: newName });
		} catch (error: any) {
			message(error, { title: $t("profile_manager.rename.failed"), buttons: { ok: $t("dialog.ok") } });
			console.error(error);
		}

		// Update folders structure
		const newFolder = newName.includes("/") ? newName.split("/")[0] : "";

		const index = folders[oldFolder].indexOf(entry);
		if (index != -1) {
			folders[oldFolder].splice(index, 1);
			if (folders[oldFolder].length == 0 && oldFolder != "") delete folders[oldFolder];
		}

		entry.name = newName;
		if (folders[newFolder]) folders[newFolder].push(entry);
		else folders[newFolder] = [entry];

		folders = folders;
		renamingProfile = null;
	}
	$: if (renameInput) renameInput.focus();

	async function duplicateProfile(id: string) {
		const entry = Object.values(folders)
			.flat()
			.find((e) => e.id == id);
		if (!entry) return;

		let newName = entry.name + $t("profile_manager.duplicate.suffix");

		// Check if a profile with the new name already exists
		const allProfiles = Object.values(folders).flat();
		let counter = 1;
		while (allProfiles.some((e) => e.name == newName)) {
			counter++;
			newName = `${entry.name}${$t("profile_manager.duplicate.suffix")} ${counter}`;
		}

		// TODO: confirm this against your actual duplicate_profile signature - `rename_profile`
		// with `retain: true` no longer makes sense now that id and name are decoupled.
		await invoke("duplicate_profile", { device: device.id, id, newName });
		await getProfiles(device);
	}

	let oldValue: string;
	$: {
		if (value == "opendeck_edit_profiles") {
			if (oldValue) showPopup = true;
			value = oldValue;
		} else if (value && value != oldValue && (!profile || profile.id != value)) {
			setProfile(value);
			oldValue = value;
		}
	}

	let showPopup: boolean = false;
	let nameInput: HTMLInputElement;

	async function createProfile() {
		if (!nameInput.checkValidity() || !nameInput.value) return;
		const entry: ProfileEntry = await invoke("create_profile", { device: device.id, name: nameInput.value });
		const folder = entry.name.includes("/") ? entry.name.split("/")[0] : "";
		if (folders[folder]) folders[folder].push(entry);
		else folders[folder] = [entry];
		folders = folders;
		await setProfile(entry.id);
		nameInput.value = "";
		showPopup = false;
	}

	let showApplicationManager: boolean = false;
	let applications: string[];
	let applicationProfiles: { [appName: string]: { [device: string]: string } };
	(async () => {
		applications = await invoke("get_applications");
		applicationProfiles = await invoke("get_application_profiles");
	})();
	listen("applications", ({ payload }: { payload: string[] }) => (applications = payload));
	let applicationsAddAppName: string = "opendeck_select_application";
	let applicationsAddProfile: string = "opendeck_select_profile";
	$: {
		if (applicationsAddAppName != "opendeck_select_application" && applicationsAddProfile != "opendeck_select_profile") {
			applicationProfiles[applicationsAddAppName] ||= {};
			applicationProfiles[applicationsAddAppName][device.id] = applicationsAddProfile;
			applicationsAddAppName = "opendeck_select_application";
			applicationsAddProfile = "opendeck_select_profile";
		}
	}
	$: {
		if (applicationProfiles) {
			applicationProfiles = Object.fromEntries(
				Object.entries(applicationProfiles).filter(([_, devices]) => Object.values(devices).filter((v) => v).length != 0),
			);
			invoke("set_application_profiles", { value: applicationProfiles });
		}
	}

	let measure: HTMLSpanElement;
	let selectWidth = 0;
	$: if (value && measure) {
		const entry = Object.values(folders)
			.flat()
			.find((e) => e.id == value);
		const name = entry?.name ?? "";
		measure.textContent = name.includes("/") ? name.split("/")[1] : name;
		selectWidth = measure.offsetWidth + 18;
	}
</script>

<div class="select-profile-wrapper">
	<span bind:this={measure} class="invisible fixed whitespace-pre pointer-events-none" aria-hidden="true"></span>
	<select bind:value style:width="{selectWidth}px" aria-label={$t("profile_manager.label")}>
		{#each Object.entries(folders).sort() as [id, profiles]}
			{#if id && profiles.length}
				<optgroup label={id}>
					{#each profiles.sort((a, b) => a.name.localeCompare(b.name)) as profile}
						<option value={profile.id}>{profile.name.split("/")[1]}</option>
					{/each}
				</optgroup>
			{:else}
				{#each profiles.sort((a, b) => a.name.localeCompare(b.name)) as profile}
					<option value={profile.id}>{profile.name}</option>
				{/each}
			{/if}
		{/each}
		<option value="opendeck_edit_profiles">{$t("profile_manager.edit")}</option>
	</select>
</div>

<svelte:window
	on:keydown={(event) => {
		if (event.key == "Escape") {
			if (showApplicationManager) showApplicationManager = false;
			else if (renamingProfile) renamingProfile = null;
			else showPopup = false;
		}
	}}
/>

<Popup show={showPopup} label="{device.name} {$t('profile_manager.profiles')}">
	<button class="mr-1 float-right text-xl text-neutral-300" on:click={() => (showPopup = false)} aria-label={$t("settings.close")}>✕</button>
	<h2 class="text-xl font-semibold text-neutral-300">{device.name}</h2>

	<div class="flex flex-row mt-2 mb-1">
		<input
			bind:this={nameInput}
			pattern="[a-zA-Z0-9_ ]+(\/[a-zA-Z0-9_ ]+)?"
			class="grow p-2 text-neutral-300 invalid:text-red-400 bg-neutral-700 border-l border-y border-neutral-600 rounded-l-lg"
			placeholder={$t("profile_manager.create.placeholder")}
			aria-label={$t("profile_manager.create.label")}
		/>

		<button
			on:click={createProfile}
			class="px-4 text-neutral-300 bg-neutral-900 hover:bg-neutral-800 transition-colors border-r border-y border-neutral-600 rounded-r-lg"
		>
			{$t("profile_manager.create")}
		</button>

		<button
			class="ml-2 px-4 flex items-center text-neutral-300 bg-neutral-900 hover:bg-neutral-800 transition-colors border border-neutral-600 rounded-lg"
			on:click={() => (showApplicationManager = true)}
			aria-label={$t("profile_manager.application_profiles")}
		>
			<Browsers size={24} />
		</button>
	</div>

	<div class="divide-y divide-neutral-500!">
		{#each Object.entries(folders).sort() as [id, profiles]}
			{#if id && profiles.length}
				<h4 class="py-2 font-bold text-lg text-neutral-300">{id}</h4>
			{/if}
			{#each profiles.sort((a, b) => a.name.localeCompare(b.name)) as profile}
				<div class="flex flex-row items-center py-2 space-x-2" class:ml-6={id} class:pl-2={id}>
					<input
						type="radio"
						bind:group={value}
						value={profile.id}
						disabled={renamingProfile === profile.id}
						id={`profile-${profile.id}`}
						aria-label={id ? profile.name.split("/")[1] : profile.name}
					/>
					{#if profile.id === renamingProfile}
						<!-- prettier-ignore -->
						<input
							bind:this={renameInput}
							bind:value={newName}
							pattern="[a-zA-Z0-9_ ]+(\/[a-zA-Z0-9_ ]+)?"
							class="grow px-2 py-1 text-neutral-300 invalid:text-red-400 bg-neutral-700 rounded"
							placeholder='Profile name or "folder/name"'
							on:keydown={(e) => {
								if (e.key === "Enter") saveRenamedProfile(profile.id);
							}}
						/>
						<button on:click={() => saveRenamedProfile(profile.id)} title={$t("profile_manager.save")} aria-label={$t("profile_manager.save")}>
							<FloppyDisk size="20" class="text-green-500" />
						</button>
					{:else}
						<label class="grow text-neutral-400" for={`profile-${profile.id}`}>{id ? profile.name.split("/")[1] : profile.name}</label>
						<button on:click={() => duplicateProfile(profile.id)} title={$t("profile_manager.duplicate")} aria-label={$t("profile_manager.duplicate")}>
							<Copy size="20" class="text-neutral-400" />
						</button>
						{#if profile.id !== value}
							<button
								on:click={() => {
									renamingProfile = profile.id;
									newName = profile.name;
								}}
								title={$t("profile_manager.rename")}
								aria-label={$t("profile_manager.rename")}
							>
								<Pencil size="20" class="text-neutral-400" />
							</button>
							<button on:click={() => deleteProfile(profile.id)} title={$t("profile_manager.delete")} aria-label={$t("profile_manager.delete")}>
								<Trash size="20" class="text-neutral-400" />
							</button>
						{/if}
					{/if}
				</div>
			{/each}
		{/each}
	</div>
</Popup>

<Popup show={showApplicationManager} label={$t("profile_manager.application_profiles")}>
	<svelte:fragment slot="header">
		<button class="mr-1 float-right text-xl text-neutral-300" on:click={() => (showApplicationManager = false)} aria-label={$t("settings.close")}>✕</button>
		<h2 class="text-xl font-semibold text-neutral-300">{device.name}</h2>
		<span class="text-sm text-neutral-400">{$t("profile_manager.application_profiles.hint.1")}</span>
		<span class="text-sm text-neutral-400">{$t("profile_manager.application_profiles.hint.2")}</span>
	</svelte:fragment>

	<table class="w-full text-neutral-300 divide-y divide-neutral-500!">
		{#each Object.entries(applicationProfiles).sort( (a, b) => (a[0] === "opendeck_default" ? -1 : b[0] === "opendeck_default" ? 1 : a[0].localeCompare(b[0])), ) as [appName, devices]}
			{#if devices[device.id]}
				<tr class="h-12">
					<td>{appName === "opendeck_default" ? $t("profile_manager.default_profile") : appName}:</td>
					<td class="select-wrapper">
						<select
							bind:value={applicationProfiles[appName][device.id]}
							class="w-full"
							aria-label={$t("profile_manager.application_profiles.aria", {
								name: appName === "opendeck_default" ? $t("profile_manager.default_profile") : appName,
							})}
						>
							{#each Object.entries(folders) as [id, profiles]}
								{#if id && profiles.length}
									<optgroup label={id}>
										{#each profiles as profile}
											<option value={profile.id}>{profile.name.split("/")[1]}</option>
										{/each}
									</optgroup>
								{:else}
									{#each profiles as profile}
										<option value={profile.id}>{profile.name}</option>
									{/each}
								{/if}
							{/each}
							<option disabled>──────────</option>
							<option value={undefined}>{$t("profile_manager.remove_application")}</option>
						</select>
					</td>
				</tr>
			{/if}
		{/each}
		<tr class="h-12">
			<td class="w-48 select-wrapper">
				<select bind:value={applicationsAddAppName} class="w-full" aria-label={$t("profile_manager.select_application")}>
					<option selected disabled value="opendeck_select_application">{$t("profile_manager.select_application.placeholder")}</option>
					{#if !applicationProfiles["opendeck_default"] || !applicationProfiles["opendeck_default"][device.id]}
						<option value="opendeck_default">{$t("profile_manager.default_profile")}</option>
						{#if applications.filter((appName) => !applicationProfiles[appName] || !applicationProfiles[appName][device.id]).length > 0}
							<option disabled>──────────</option>
						{/if}
					{/if}
					{#each applications as appName}
						{#if !applicationProfiles[appName] || !applicationProfiles[appName][device.id]}
							<option value={appName}>{appName}</option>
						{/if}
					{/each}
				</select>
			</td>
			<td class="w-96 select-wrapper">
				<select bind:value={applicationsAddProfile} class="w-full" aria-label={$t("profile_manager.select_profile")}>
					<option selected disabled value="opendeck_select_profile">{$t("profile_manager.select_profile.placeholder")}</option>
					{#each Object.entries(folders) as [id, profiles]}
						{#if id && profiles.length}
							<optgroup label={id}>
								{#each profiles as profile}
									<option value={profile.id}>{profile.name.split("/")[1]}</option>
								{/each}
							</optgroup>
						{:else}
							{#each profiles as profile}
								<option value={profile.id}>{profile.name}</option>
							{/each}
						{/if}
					{/each}
				</select>
			</td>
		</tr>
	</table>
</Popup>