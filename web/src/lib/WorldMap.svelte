<script lang="ts">
	import { onMount } from 'svelte';
	import { prefersReducedMotion } from 'svelte/motion';
	import type { Map as MapLibreMap, MapGeoJSONFeature } from 'maplibre-gl';
	import type { Feature, FeatureCollection, Geometry } from 'geojson';
	import { type Country, type CountryProgress, type Location } from './types';
	import { SPIN_MS } from './state.svelte';
	import { bbox, centre, fittingZoom, unwrap } from './map/geometry';
	import { facetImage, sparkleImage } from './map/images';
	import { SOURCE, SPARKLES, fillOpacity, mapStyle, paint, twinkle } from './map/style';

	interface Props {
		countries: Country[];
		progress: CountryProgress[];
		selected: string | null;
		location: Location | null;
		projection: 'globe' | 'mercator';
		/** Changes whenever the colour scheme does, so the map can repaint. */
		theme: string;
		/** Turns the globe until it goes false; the camera then flies to `selected`. */
		spinning: boolean;
		onselect: (code: string, name: string, available: boolean) => void;
	}

	let { countries, progress, selected, location, projection, theme, spinning, onselect }: Props = $props();

	let container: HTMLDivElement;
	let map = $state<MapLibreMap | null>(null);
	let hover = $state<{ x: number; y: number; code: string; name: string } | null>(null);
	let hoveredId: string | null = null;
	let selectedId: string | null = null;
	let coloured: string[] = [];
	/** Each country's bounding box, to fly to it when it is selected. */
	let bounds = new Map<string, [number, number, number, number]>();
	/** Each country's shape, for the diamond overlay on completed ones. */
	let shapes = new Map<string, Feature>();
	/** Follows the setting live, so turning it on mid-visit stops the sparkles. */
	const still = $derived(prefersReducedMotion.current);

	const byCode = $derived(new Map(progress.map((p) => [p.code, p])));
	const hoverInfo = $derived(hover ? { country: countries.find((c) => c.code === hover!.code), progress: byCode.get(hover.code) } : null);

	onMount(() => {
		let disposed = false;
		let instance: MapLibreMap | null = null;

		(async () => {
			const [{ Map: MapLibre, setWorkerUrl }, worker, topojson, atlas] = await Promise.all([
				import('maplibre-gl'),
				// MapLibre locates its worker next to its own file, which no longer holds once
				// bundled, so let Vite bundle the worker and hand over the resulting URL.
				import('maplibre-gl/dist/maplibre-gl-worker.mjs?worker&url'),
				import('topojson-client'),
				import('world-atlas/countries-50m.json')
			]);
			if (disposed) return;
			setWorkerUrl(worker.default);

			// The atlas identifies countries by ISO numeric code; the API speaks alpha-2.
			const byNumeric = new Map(countries.filter((c) => c.numeric).map((c) => [c.numeric, c]));
			const topology = (atlas as any).default ?? atlas;
			const world = topojson.feature(topology, topology.objects.countries) as unknown as FeatureCollection<Geometry, { name: string; code?: string }>;
			const features: Feature[] = world.features
				.filter((f) => f.id !== '010') // Antarctica
				.map((f) => {
					const known = byNumeric.get(String(f.id));
					return { ...f, geometry: unwrap(f.geometry), properties: { code: known?.code ?? `x-${f.properties.name}`, name: known?.name ?? f.properties.name } };
				});

			for (const f of features) {
				bounds.set(String(f.properties!.code), bbox(f.geometry));
				shapes.set(String(f.properties!.code), f);
			}

			const themed = paint() as any;

			const center: [number, number] = location ? [location.lng, location.lat] : [10, 30];
			instance = new MapLibre({
				container,
				center,
				zoom: fittingZoom(container, center[1]),
				minZoom: 0.6,
				maxZoom: 6,
				attributionControl: { compact: true, customAttribution: 'Borders: Natural Earth' },
				dragRotate: false,
				style: mapStyle(projection, features, themed)
			});

			const m = instance;
			m.touchZoomRotate.disableRotation();
			m.keyboard.disableRotation();

			// On the globe, MapLibre still reports countries under points in the empty space
			// around the sphere. Such a point has no location of its own: unprojecting it snaps
			// to the globe's edge, so projecting back lands elsewhere.
			const onMap = (point: { x: number; y: number }) => {
				const back = m.project(m.unproject([point.x, point.y]));
				return Math.hypot(back.x - point.x, back.y - point.y) < 1;
			};
			const clearHover = () => {
				if (hoveredId) m.setFeatureState({ source: SOURCE, id: hoveredId }, { hover: false });
				hoveredId = null;
				hover = null;
				m.getCanvas().style.cursor = '';
			};

			m.on('mousemove', 'fill', (e) => {
				const feature = e.features?.[0] as MapGeoJSONFeature | undefined;
				if (!feature || !onMap(e.point)) return clearHover();
				const id = String(feature.id);
				if (id !== hoveredId) {
					if (hoveredId) m.setFeatureState({ source: SOURCE, id: hoveredId }, { hover: false });
					m.setFeatureState({ source: SOURCE, id }, { hover: true });
					hoveredId = id;
				}
				m.getCanvas().style.cursor = 'pointer';
				hover = { x: e.point.x, y: e.point.y, code: id, name: feature.properties.name };
			});
			m.on('mouseleave', 'fill', clearHover);
			// The pointer stops matching what is under it once the map moves.
			m.on('movestart', () => (hover = null));
			m.on('click', 'fill', (e) => {
				const feature = e.features?.[0];
				if (!feature || !onMap(e.point)) return;
				const code = String(feature.id);
				onselect(code, feature.properties.name, byCode.has(code));
			});
			m.on('load', () => {
				// The diamond's facets and sparkle are drawn here rather than loaded.
				const css = getComputedStyle(document.documentElement);
				m.addImage('facets', facetImage(css.getPropertyValue('--completed-facet').trim()), { pixelRatio: 2 });
				m.addImage('sparkle', sparkleImage(css.getPropertyValue('--completed-edge').trim()), { pixelRatio: 2 });
				// The attribution starts expanded and would cover the legend on small screens.
				const attribution = container.querySelector('.maplibregl-ctrl-attrib');
				attribution?.classList.remove('maplibregl-compact-show');
				attribution?.removeAttribute('open');
				if (!disposed) map = m;
			});

		})();

		return () => {
			disposed = true;
			instance?.remove();
		};
	});

	// Colour the countries by progress.
	$effect(() => {
		if (!map) return;
		for (const id of coloured) map.removeFeatureState({ source: SOURCE, id }, 'status');
		coloured = progress.map((entry) => entry.code);
		for (const entry of progress) {
			map.setFeatureState({ source: SOURCE, id: entry.code }, { status: entry.status });
		}
	});

	$effect(() => {
		if (!map) return;
		if (selectedId) map.setFeatureState({ source: SOURCE, id: selectedId }, { selected: false });
		if (selected) map.setFeatureState({ source: SOURCE, id: selected }, { selected: true });
		selectedId = selected;
		map.setPaintProperty('fill', 'fill-opacity', fillOpacity(selected !== null) as any);

		// The camera's one owner: fly to the selected country, or else the globe around the user.
		// A spin has the camera until it lands.
		if (spinning) return;
		const box = selected ? bounds.get(selected) : null;
		const animate = !still;
		if (box) {
			const el = map.getContainer();
			const pad = Math.min(el.clientWidth, el.clientHeight) * 0.3;
			map.fitBounds(box, { padding: pad, maxZoom: 4.5, duration: animate ? 1200 : 0 });
		} else if (location) {
			const zoom = fittingZoom(map.getContainer(), location.lat);
			map.easeTo({ center: [location.lng, location.lat], zoom, duration: animate ? 1200 : 0 });
		}
	});

	// A spin: zoom out to the whole globe and turn it until the spin lands, when the camera
	// effect above flies to the chosen country.
	$effect(() => {
		if (!map || !spinning || still) return;
		const m = map;
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
	});

	// Completed countries get the diamond: facets over their fill, and sparkles.
	$effect(() => {
		if (!map) return;
		const done = progress.filter((entry) => entry.status === 'completed').flatMap((entry) => shapes.get(entry.code) ?? []);
		(map.getSource('diamonds') as any)?.setData({ type: 'FeatureCollection', features: done });
		(map.getSource('sparkles') as any)?.setData({
			type: 'FeatureCollection',
			features: done.map((f) => ({ type: 'Feature', properties: {}, geometry: { type: 'Point', coordinates: centre(f.geometry) } }))
		});
		if (still || !done.length) return;
		// Twinkle: each sparkle flashes on its own beat, growing and brightening quickly and
		// then shrinking back. About 20 updates a second, nothing while the tab is hidden.
		const m = map;
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
	});

	$effect(() => {
		void theme;
		if (!map) return;
		const css = getComputedStyle(document.documentElement);
		if (map.hasImage('facets')) map.updateImage('facets', facetImage(css.getPropertyValue('--completed-facet').trim()));
		if (map.hasImage('sparkle')) map.updateImage('sparkle', sparkleImage(css.getPropertyValue('--completed-edge').trim()));
		for (const [layer, properties] of Object.entries(paint())) {
			for (const [name, value] of Object.entries(properties)) map.setPaintProperty(layer, name as any, value);
		}
	});

	$effect(() => {
		if (!map) return;
		map.setProjection({ type: projection });
	});

	$effect(() => {
		if (!map) return;
		const source = map.getSource('here') as any;
		source?.setData({
			type: 'FeatureCollection',
			features: location
				? [{ type: 'Feature', properties: {}, geometry: { type: 'Point', coordinates: [location.lng, location.lat] } }]
				: []
		});
	});
</script>

<div class="map" bind:this={container}></div>

{#if hover && hoverInfo}
	<div class="tip" style:left="{hover.x}px" style:top="{hover.y}px">
		<strong>{hoverInfo.country?.flag ?? ''} {hover.name}</strong>
		{#if hoverInfo.progress}
			<span>{hoverInfo.progress.visited} of {hoverInfo.progress.places} {hoverInfo.progress.places === 1 ? 'place' : 'places'} visited</span>
		{:else}
			<span>Not found in this area</span>
		{/if}
	</div>
{/if}

<style>
	.map {
		position: absolute;
		inset: 0;
		background: radial-gradient(circle at 50% 40%, var(--space) 0%, var(--bg) 70%);
	}

	.tip {
		position: absolute;
		z-index: 2;
		transform: translate(14px, 14px);
		pointer-events: none;
		display: grid;
		padding: 8px 10px;
		border-radius: 10px;
		background: var(--panel-raised);
		border: 1px solid var(--line);
		box-shadow: 0 8px 24px var(--shadow);
		font-size: var(--text-sm);
		white-space: nowrap;
	}

	.tip span {
		color: var(--muted);
	}

	.map :global(.maplibregl-ctrl-attrib) {
		background: var(--glass);
		color: var(--muted);
	}

	.map :global(.maplibregl-ctrl-attrib a) {
		color: var(--muted);
	}
</style>
