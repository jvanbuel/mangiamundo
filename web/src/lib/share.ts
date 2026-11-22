import { toast } from './state.svelte';
import type { ShareLink } from './types';

/** A link with its card attached, ready for the share sheet. */
export interface Prepared {
	link: ShareLink;
	file: File | null;
}

/** Fetches the card for a link the server made. Without it the link is shared alone. */
export async function prepare(link: ShareLink): Promise<Prepared> {
	const image = await fetch(link.image_url)
		.then((r) => (r.ok ? r.blob() : null))
		.catch(() => null);
	return { link, file: image && new File([image], 'mangiamundo.png', { type: 'image/png' }) };
}

/**
 * Opens the share sheet with the card, text and link, or copies the link where there is
 * no share sheet. Returns false when the browser refused because the tap was too long
 * ago (the card took a while to load): tapping again shares straight away.
 */
export async function share({ link, file }: Prepared): Promise<boolean> {
	const { url, text } = link;
	try {
		if (typeof navigator.share !== 'function') {
			// Desktop browsers without a share sheet.
			await navigator.clipboard.writeText(url);
			toast(null, 'Link copied', 'Paste it anywhere to share.');
		} else if (file && navigator.canShare?.({ files: [file], text, url })) await navigator.share({ files: [file], text, url });
		else await navigator.share({ text, url });
	} catch (e) {
		const name = (e as Error).name;
		if (name === 'NotAllowedError') return false;
		if (name !== 'AbortError') toast(null, 'Could not share', (e as Error).message);
	}
	return true;
}
