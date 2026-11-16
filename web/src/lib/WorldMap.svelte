<script lang="ts">
	import { onMount } from 'svelte';
	import type { Map as MapLibreMap, MapGeoJSONFeature } from 'maplibre-gl';
	import type { Feature, FeatureCollection, Geometry } from 'geojson';
	import { statusOf, type Country, type CountryProgress, type Location } from './types';

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

	const SOURCE = 'countries';
	let container: HTMLDivElement;
	let map = $state<MapLibreMap | null>(null);
	let hover = $state<{ x: number; y: number; code: string; name: string } | null>(null);
	let hoveredId: string | null = null;
	let selectedId: string | null = null;
	let coloured: string[] = [];

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

	const featureState = (key: string, fallback: unknown) => ['coalesce', ['feature-state', key], fallback];
	const flag = (key: string) => ['boolean', featureState(key, false), false];

	/** Paint properties that depend on the theme, read from the CSS custom properties. */
	function paint(): Record<string, Record<string, unknown>> {
		const css = getComputedStyle(document.documentElement);
		const color = (name: string) => css.getPropertyValue(name).trim();
		return {
			ocean: { 'background-color': color('--ocean') },
			fill: {
				'fill-color': [
					'match', featureState('status', 'none'),
					'available', color('--available'),
					'discovered', color('--discovered'),
					'completed', color('--completed'),
					color('--land')
				]
			},
			borders: { 'line-color': ['case', flag('selected'), color('--text'), color('--ocean')] },
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

			const themed = paint() as any;

			instance = new MapLibre({
				container,
				center: location ? [location.lng, location.lat] : [10, 30],
				zoom: 1.4,
				minZoom: 0.6,
				maxZoom: 6,
				attributionControl: { compact: true, customAttribution: 'Borders: Natural Earth' },
				dragRotate: false,
				style: {
					version: 8,
					projection: { type: projection },
					sources: {
						[SOURCE]: { type: 'geojson', data: { type: 'FeatureCollection', features }, promoteId: 'code' },
						here: { type: 'geojson', data: { type: 'FeatureCollection', features: [] } }
					},
					layers: [
						{ id: 'ocean', type: 'background', paint: themed.ocean },
						{
							id: 'fill',
							type: 'fill',
							source: SOURCE,
							paint: {
								...themed.fill,
								'fill-opacity': ['case', flag('hover'), 1, 0.86] as any
							}
						},
						{
							id: 'borders',
							type: 'line',
							source: SOURCE,
							paint: {
								...themed.borders,
								'line-width': ['case', flag('selected'), 2, 0.6] as any
							}
						},
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
			map.setFeatureState({ source: SOURCE, id: entry.code }, { status: statusOf(entry) });
		}
	});

	$effect(() => {
		if (!map) return;
		if (selectedId) map.setFeatureState({ source: SOURCE, id: selectedId }, { selected: false });
		if (selected) map.setFeatureState({ source: SOURCE, id: selected }, { selected: true });
		selectedId = selected;
	});

	$effect(() => {
		void theme;
		if (!map) return;
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
		if (location) map.easeTo({ center: [location.lng, location.lat], duration: 1200 });
	});
</script>

<div class="map" bind:this={container}></div>

{#if hover && hoverInfo}
	<div class="tip" style:left="{hover.x}px" style:top="{hover.y}px">
		<strong>{hoverInfo.country?.flag ?? ''} {hover.name}</strong>
		{#if hoverInfo.progress}
			<span>{hoverInfo.progress.visited} of {hoverInfo.progress.places} {hoverInfo.progress.places === 1 ? 'place' : 'places'} visited</span>
		{:else}
			<span>Not found in this city</span>
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
		font-size: 13px;
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
