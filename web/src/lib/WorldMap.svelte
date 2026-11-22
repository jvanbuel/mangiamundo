<script lang="ts">
	import { onMount } from 'svelte';
	import { prefersReducedMotion } from 'svelte/motion';
	import type { Map as MapLibreMap, MapGeoJSONFeature } from 'maplibre-gl';
	import type { Feature, FeatureCollection, Geometry } from 'geojson';
	import { type Country, type CountryProgress, type Location } from './types';

	interface Props {
		countries: Country[];
		progress: CountryProgress[];
		selected: string | null;
		location: Location | null;
		projection: 'globe' | 'mercator';
		/** Changes whenever the colour scheme does, so the map can repaint. */
		theme: string;
		onselect: (code: string, name: string, available: boolean) => void;
	}

	let { countries, progress, selected, location, projection, theme, onselect }: Props = $props();

	/** Zoom at which the globe's diameter fills most of the map's shorter side. */
	function fittingZoom(el: HTMLElement, lat: number, fill = 0.95): number {
		const diameter = Math.min(el.clientWidth, el.clientHeight) * fill;
		// MapLibre draws the globe with radius 512 * 2^zoom / (2π cos(lat)).
		return Math.log2((diameter * Math.PI * Math.cos((lat * Math.PI) / 180)) / 512);
	}

	const SOURCE = 'countries';
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

	/**
	 * The atlas stores countries that cross the antimeridian (Russia, Fiji, ...) as one
	 * ring whose longitude jumps from +180 to -180. A flat renderer would draw that jump
	 * as a stripe across the whole world, so make each ring continuous instead, letting
	 * it run past 180 degrees. MapLibre wraps such geometry correctly.
	 */
	function unwrap(geometry: Geometry): Geometry {
		const ring = (points: number[][]) => {
			let shift = 0;
			return points.map(([lng, lat], i) => {
				if (i > 0) {
					const jump = lng - points[i - 1][0];
					if (jump > 180) shift -= 360;
					else if (jump < -180) shift += 360;
				}
				return [lng + shift, lat];
			});
		};
		if (geometry.type === 'Polygon') return { ...geometry, coordinates: geometry.coordinates.map(ring) };
		if (geometry.type === 'MultiPolygon') {
			return { ...geometry, coordinates: geometry.coordinates.map((polygon) => polygon.map(ring)) };
		}
		return geometry;
	}

	/** [west, south, east, north] of a (multi)polygon whose rings were unwrapped. */
	function bbox(geometry: Geometry): [number, number, number, number] {
		const box: [number, number, number, number] = [Infinity, Infinity, -Infinity, -Infinity];
		const polygons = geometry.type === 'Polygon' ? [geometry.coordinates] : geometry.type === 'MultiPolygon' ? geometry.coordinates : [];
		for (const polygon of polygons) {
			for (const [lng, lat] of polygon[0]) {
				box[0] = Math.min(box[0], lng);
				box[1] = Math.min(box[1], lat);
				box[2] = Math.max(box[2], lng);
				box[3] = Math.max(box[3], lat);
			}
		}
		return box;
	}

	/** Where a country's sparkle sits: the area-weighted centre of its largest ring. */
	function centre(geometry: Geometry): [number, number] {
		const rings = geometry.type === 'Polygon' ? [geometry.coordinates[0]] : geometry.type === 'MultiPolygon' ? geometry.coordinates.map((p) => p[0]) : [];
		let best: [number, number] = [0, 0];
		let largest = -1;
		for (const ring of rings) {
			let area = 0, x = 0, y = 0;
			for (let i = 0; i < ring.length - 1; i++) {
				const [x0, y0] = ring[i], [x1, y1] = ring[i + 1];
				const cross = x0 * y1 - x1 * y0;
				area += cross;
				x += (x0 + x1) * cross;
				y += (y0 + y1) * cross;
			}
			if (Math.abs(area) > largest && area !== 0) {
				largest = Math.abs(area);
				best = [x / (3 * area), y / (3 * area)];
			}
		}
		return best;
	}

	/** A four-pointed star in the diamond's edge colour with a white heart and a soft
	    glow: the sparkle. Coloured rather than white, so it shows on the pale diamond. */
	function sparkleImage(edge: string): ImageData {
		const size = 64, c = size / 2;
		const ctx = document.createElement('canvas').getContext('2d')!;
		ctx.canvas.width = ctx.canvas.height = size;
		const dirs = [[0, -1], [1, 0], [0, 1], [-1, 0]];
		const star = (tip: number, waist: number) => {
			ctx.beginPath();
			ctx.moveTo(c, c - tip);
			dirs.forEach(([dx, dy], i) => {
				const [nx, ny] = dirs[(i + 1) % 4];
				// Curve in towards the middle between one tip and the next.
				ctx.quadraticCurveTo(c + (dx + nx) * waist, c + (dy + ny) * waist, c + nx * tip, c + ny * tip);
			});
			ctx.closePath();
		};
		star(28, 7);
		ctx.shadowColor = '#ffffff';
		ctx.shadowBlur = 6;
		ctx.fillStyle = edge;
		ctx.fill();
		ctx.shadowBlur = 0;
		star(13, 3);
		ctx.fillStyle = '#ffffff';
		ctx.fill();
		return ctx.getImageData(0, 0, size, size);
	}

	/** Facets cut into a completed country: crossing lines and a few bright glints. */
	function facetImage(line: string): ImageData {
		const size = 40;
		const ctx = document.createElement('canvas').getContext('2d')!;
		ctx.canvas.width = ctx.canvas.height = size;
		ctx.strokeStyle = line;
		ctx.lineWidth = 2;
		ctx.beginPath();
		ctx.moveTo(0, 0); ctx.lineTo(size, size);
		ctx.moveTo(size, 0); ctx.lineTo(0, size);
		ctx.moveTo(size / 2, 0); ctx.lineTo(size, size / 2); ctx.lineTo(size / 2, size); ctx.lineTo(0, size / 2); ctx.closePath();
		ctx.stroke();
		ctx.fillStyle = 'rgb(255 255 255 / 0.8)';
		ctx.beginPath();
		ctx.moveTo(size / 2, 2); ctx.lineTo(size * 0.62, size * 0.22); ctx.lineTo(size / 2, size * 0.3); ctx.closePath();
		ctx.moveTo(4, size / 2); ctx.lineTo(size * 0.16, size * 0.42); ctx.lineTo(size * 0.2, size / 2); ctx.closePath();
		ctx.fill();
		return ctx.getImageData(0, 0, size, size);
	}

	/** Fill opacity: with a country selected, the others step back. */
	const fillOpacity = (focused: boolean) =>
		focused ? ['case', flag('selected'), 1, flag('hover'), 0.8, 0.4] : ['case', flag('hover'), 1, 0.86];

	const SPARKLES: [string, number, [number, number]][] = [
		['sparkle-a', 1.6, [0, 0]],
		['sparkle-b', 1, [26, -22]],
		['sparkle-c', 0.8, [-30, 24]]
	];

	/** A sparkle's brightness, 0 to 1, at a time in seconds: a quick flash every 2.4s,
	    each sparkle a third of a beat after the last. */
	function twinkle(seconds: number, index: number): number {
		const wave = Math.max(0, Math.sin(seconds * 2.6 + index * 2.1));
		return wave * wave;
	}

	const featureState = (key: string, fallback: unknown) => ['coalesce', ['feature-state', key], fallback];
	const flag = (key: string) => ['boolean', featureState(key, false), false];

	/** Paint properties that depend on the theme, read from the CSS custom properties. */
	function paint(): Record<string, Record<string, unknown>> {
		const css = getComputedStyle(document.documentElement);
		const color = (name: string) => css.getPropertyValue(name).trim();
		return {
			ocean: { 'background-color': color('--ocean') },
			land: { 'fill-color': color('--land') },
			fill: {
				'fill-color': [
					'match', featureState('status', 'none'),
					'available', color('--available'),
					'discovered', color('--discovered'),
					'platinum', color('--platinum'),
					'completed', color('--completed'),
					color('--land')
				]
			},
			borders: {
				'line-color': [
					'case', flag('selected'), color('--text'),
					['==', featureState('status', 'none'), 'available'], color('--available-edge'),
					['==', featureState('status', 'none'), 'discovered'], color('--discovered-edge'),
					['==', featureState('status', 'none'), 'platinum'], color('--platinum-edge'),
					['==', featureState('status', 'none'), 'completed'], color('--completed-edge'),
					color('--map-border')
				]
			},
			'here-halo': { 'circle-color': color('--text') },
			here: { 'circle-color': color('--text'), 'circle-stroke-color': color('--panel') }
		};
	}

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
				style: {
					version: 8,
					projection: { type: projection },
					sources: {
						[SOURCE]: { type: 'geojson', data: { type: 'FeatureCollection', features }, promoteId: 'code' },
						here: { type: 'geojson', data: { type: 'FeatureCollection', features: [] } },
						// Completed countries and their sparkles, kept in sources of their own because
						// patterns and symbols can't follow feature state.
						diamonds: { type: 'geojson', data: { type: 'FeatureCollection', features: [] } },
						sparkles: { type: 'geojson', data: { type: 'FeatureCollection', features: [] } }
					},
					layers: [
						{ id: 'ocean', type: 'background', paint: themed.ocean },
						// Solid land under the coloured fill, so countries that step back fade to land, not sea.
						{ id: 'land', type: 'fill', source: SOURCE, paint: themed.land },
						{
							id: 'fill',
							type: 'fill',
							source: SOURCE,
							paint: {
								...themed.fill,
								'fill-opacity': fillOpacity(false) as any
							}
						},
						{ id: 'facets', type: 'fill', source: 'diamonds', paint: { 'fill-pattern': 'facets' } },
						{
							id: 'borders',
							type: 'line',
							source: SOURCE,
							paint: {
								...themed.borders,
								'line-width': [
									'case', flag('selected'), 2.5,
									['==', featureState('status', 'none'), 'completed'], 2,
									['match', featureState('status', 'none'), ['available', 'discovered', 'platinum'], true, false], 1,
									0.6
								] as any
							}
						},
						// Three sparkles per diamond, offset in pixels so small countries get them too.
						...SPARKLES.map(([id, size, offset]) => ({
							id,
							type: 'symbol' as const,
							source: 'sparkles',
							layout: { 'icon-image': 'sparkle', 'icon-size': size, 'icon-offset': offset, 'icon-allow-overlap': true, 'icon-ignore-placement': true }
						})),
						{
							id: 'here-halo',
							type: 'circle',
							source: 'here',
							paint: { ...themed['here-halo'], 'circle-radius': 13, 'circle-opacity': 0.25 }
						},
						{
							id: 'here',
							type: 'circle',
							source: 'here',
							paint: { ...themed.here, 'circle-radius': 5, 'circle-stroke-width': 2 }
						}
					]
				}
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
