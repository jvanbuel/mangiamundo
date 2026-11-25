import type { MapOptions } from 'maplibre-gl';
import type { Feature } from 'geojson';

export const SOURCE = 'countries';

/** Fill opacity: with a country selected, the others step back. */
export const fillOpacity = (focused: boolean) =>
	focused ? ['case', flag('selected'), 1, flag('hover'), 0.8, 0.4] : ['case', flag('hover'), 1, 0.86];

export const SPARKLES: [string, number, [number, number]][] = [
	['sparkle-a', 1.6, [0, 0]],
	['sparkle-b', 1, [26, -22]],
	['sparkle-c', 0.8, [-30, 24]]
];

/** A sparkle's brightness, 0 to 1, at a time in seconds: a quick flash every 2.4s,
    each sparkle a third of a beat after the last. */
export function twinkle(seconds: number, index: number): number {
	const wave = Math.max(0, Math.sin(seconds * 2.6 + index * 2.1));
	return wave * wave;
}

export const featureState = (key: string, fallback: unknown) => ['coalesce', ['feature-state', key], fallback];
export const flag = (key: string) => ['boolean', featureState(key, false), false];

/** Paint properties that depend on the theme, read from the CSS custom properties. */
export function paint(): Record<string, Record<string, unknown>> {
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

/** The map's sources and layers: the countries, the diamonds and their sparkles, and the user. */
export function mapStyle(projection: 'globe' | 'mercator', features: Feature[], themed: any): MapOptions['style'] {
	return {
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
	};
}
