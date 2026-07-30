import type { PageView } from "$lib/PageView.ts";

export type ProfileView = {
	device: string;

	id: string;
	name: string;

	page_count: number;
	current_page_number: number;

	pinned: PageView;
	current_page: PageView;
};
