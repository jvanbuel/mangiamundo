import type { Map as MapLibreMap } from 'maplibre-gl';
import { SPIN_MS } from '../state.svelte';
import { fittingZoom } from './geometry';
import { SPARKLES } from './style';

/** Turns the globe for a spin, zooming out to the whole of it. Returns a function that stops. */
export function spinGlobe(m: MapLibreMap): () => void {
	m.stop();
	const from = m.getCenter();
	const fromZoom = m.getZoom();
	const globe = fittingZoom(m.getContainer(), from.lat);
	const start = performance.now();
	let frame = 0;
	const tick = (now: number) => {
		const elapsed = now - start;
		const out = Math.min(1, elapsed / 500);
		// One turn per SPIN_MS, zooming out over the first half second.
		m.jumpTo({ center: [from.lng + (360 * elapsed) / SPIN_MS, from.lat], zoom: fromZoom + (globe - fromZoom) * out });
		frame = requestAnimationFrame(tick);
	};
	frame = requestAnimationFrame(tick);
	return () => cancelAnimationFrame(frame);
}

/** A sparkle's brightness, 0 to 1, at a time in seconds: a quick flash every 2.4s,
    each sparkle a third of a beat after the last. */
function twinkle(seconds: number, index: number): number {
	const wave = Math.max(0, Math.sin(seconds * 2.6 + index * 2.1));
	return wave * wave;
}

/** Twinkle: each sparkle flashes on its own beat, growing and brightening quickly and
 * then shrinking back. About 20 updates a second, nothing while the tab is hidden.
 * Returns a function that stops. */
export function twinkleSparkles(m: MapLibreMap): () => void {
	let frame = 0, last = 0;
	const tick = (now: number) => {
		frame = requestAnimationFrame(tick);
		if (now - last < 50 || document.hidden) return;
		last = now;
		SPARKLES.forEach(([id, size], i) => {
			const flash = twinkle(now / 1000, i);
			m.setPaintProperty(id, 'icon-opacity', 0.25 + 0.75 * flash);
			m.setLayoutProperty(id, 'icon-size', size * (0.55 + 0.45 * flash));
		});
	};
	frame = requestAnimationFrame(tick);
	return () => cancelAnimationFrame(frame);
}
