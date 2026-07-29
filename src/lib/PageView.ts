import type { ActionInstance } from "$lib/ActionInstance.ts";

export type PageView = {
	keys: (ActionInstance | null)[];
	encoders: (ActionInstance | null)[];
	infobars: (ActionInstance | null)[];
};