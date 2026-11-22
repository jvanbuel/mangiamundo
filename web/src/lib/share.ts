import { mapSnapshot } from './WorldMap.svelte';
import { toast } from './state.svelte';
import type { Discover } from './types';

/** What the share says, also used as the card's caption. */
export function shareText(d: Discover, place: string): { headline: string; text: string } {
	const { discovered, available } = d.world;
	return {
		headline: `${discovered} of ${available} countries tasted`,
		text: `I've tasted food from ${discovered} of the ${available} countries I can eat in ${place}. Level ${d.profile.level} on Mangiamundo.`
	};
}

/** The lit-up map with a caption underneath, as a PNG. */
async function card(map: HTMLCanvasElement, headline: string, sub: string): Promise<Blob | null> {
	const W = 1200, H = 630, strip = 150;
	const css = getComputedStyle(document.documentElement);
	const color = (name: string) => css.getPropertyValue(name).trim();
	const font = getComputedStyle(document.body).fontFamily;
	const ctx = document.createElement('canvas').getContext('2d')!;
	ctx.canvas.width = W;
	ctx.canvas.height = H;
	ctx.fillStyle = color('--bg');
	ctx.fillRect(0, 0, W, H);
	// Cover the area above the caption, cropping the map's centre.
	const scale = Math.max(W / map.width, (H - strip) / map.height);
	const w = map.width * scale, h = map.height * scale;
	ctx.save();
	ctx.beginPath();
	ctx.rect(0, 0, W, H - strip);
	ctx.clip();
	ctx.drawImage(map, (W - w) / 2, (H - strip - h) / 2, w, h);
	ctx.restore();
	ctx.fillStyle = color('--panel');
	ctx.fillRect(0, H - strip, W, strip);
	ctx.fillStyle = color('--text');
	ctx.font = `800 52px ${font}`;
	ctx.fillText(headline, 48, H - strip + 70);
	ctx.fillStyle = color('--muted');
	ctx.font = `600 30px ${font}`;
	ctx.fillText(sub, 48, H - strip + 116);
	return new Promise((resolve) => ctx.canvas.toBlob(resolve, 'image/png'));
}

/** Shares the map and progress through the system share sheet, or downloads the card. */
export async function shareProgress(d: Discover, place: string) {
	const { headline, text } = shareText(d, place);
	const url = location.origin;
	const map = await mapSnapshot();
	const blob = map && (await card(map, headline, `${place} · Level ${d.profile.level} · ${location.host}`));
	const file = blob && new File([blob], 'mangiamundo.png', { type: 'image/png' });
	try {
		if (file && navigator.canShare?.({ files: [file] })) await navigator.share({ files: [file], text, url });
		else if ('share' in navigator) await navigator.share({ text, url });
		else if (file) {
			// Desktop browsers without a share sheet: save the card instead.
			const a = document.createElement('a');
			a.href = URL.createObjectURL(file);
			a.download = file.name;
			a.click();
			URL.revokeObjectURL(a.href);
		}
	} catch (e) {
		if ((e as Error).name !== 'AbortError') toast(null, 'Could not share', (e as Error).message);
	}
}
