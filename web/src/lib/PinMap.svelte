<script lang="ts">
	import { onMount } from 'svelte';
	import MapPin from '@lucide/svelte/icons/map-pin';

	/** The point under the pin, which stays in the middle while the map moves beneath it. */
	let { point = $bindable() }: { point: { lat: number; lng: number } } = $props();

	let container: HTMLDivElement;

	onMount(() => {
		let disposed = false;
		let remove = () => {};
		(async () => {
			const [{ Map: MapLibre, setWorkerUrl }, worker] = await Promise.all([
				import('maplibre-gl'),
				import('maplibre-gl/dist/maplibre-gl-worker.mjs?worker&url')
			]);
			if (disposed) return;
			setWorkerUrl(worker.default);
			const map = new MapLibre({
				container,
				// Free street tiles from OpenStreetMap data, no key needed.
				style: 'https://tiles.openfreemap.org/styles/liberty',
				center: [point.lng, point.lat],
				zoom: 17,
				dragRotate: false,
				attributionControl: { compact: true }
			});
			// Folded, so the credits don't cover the small map.
			map.once('load', () => container.querySelector('.maplibregl-ctrl-attrib')?.classList.remove('maplibregl-compact-show'));
			map.on('move', () => {
				const c = map.getCenter();
				point = { lat: c.lat, lng: c.lng };
			});
			remove = () => map.remove();
		})();
		return () => {
			disposed = true;
			remove();
		};
	});
</script>

<div class="wrap">
	<div class="map" bind:this={container} role="application" aria-label="Map: drag it until the pin is on the restaurant"></div>
	<span class="pin" aria-hidden="true"><MapPin size={34} strokeWidth={2.5} /></span>
</div>

<style>
	.wrap {
		position: relative;
		height: 220px;
		border-radius: 12px;
		overflow: hidden;
		border: 1px solid var(--line);
	}

	.map {
		position: absolute;
		inset: 0;
	}

	/* The pin's tip marks the map's centre. */
	.pin {
		position: absolute;
		left: 50%;
		top: 50%;
		transform: translate(-50%, -100%);
		color: var(--accent);
		pointer-events: none;
		filter: drop-shadow(0 1px 1px rgb(0 0 0 / 0.4));
	}
</style>
