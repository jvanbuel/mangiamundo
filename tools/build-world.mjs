// Builds mobile/shared/world.json, the country outlines drawn by the native apps.
// Run from the repository root after `npm install` in web/:  node tools/build-world.mjs
import { readFileSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';

const require = createRequire(new URL('../web/package.json', import.meta.url));
const { feature } = require('topojson-client');
const atlas = require('world-atlas/countries-110m.json');
const countries = JSON.parse(readFileSync(new URL('../backend/data/countries.json', import.meta.url)));
const byNumeric = new Map(countries.filter((c) => c.numeric).map((c) => [c.numeric, c]));

// Rings that cross the antimeridian jump from +180 to -180; make them continuous
// (running past 180) so a flat map does not draw a stripe across the world.
function ring(points) {
	let shift = 0;
	const out = [];
	points.forEach(([lng, lat], i) => {
		if (i > 0) {
			const jump = lng - points[i - 1][0];
			if (jump > 180) shift -= 360;
			else if (jump < -180) shift += 360;
		}
		out.push(Math.round((lng + shift) * 100) / 100, Math.round(lat * 100) / 100);
	});
	return out;
}

const world = feature(atlas, atlas.objects.countries)
	.features.filter((f) => f.id !== '010') // Antarctica
	.map((f) => {
		const known = byNumeric.get(String(f.id));
		const polygons = f.geometry.type === 'Polygon' ? [f.geometry.coordinates] : f.geometry.coordinates;
		return {
			code: known?.code ?? `x-${f.properties.name}`,
			name: known?.name ?? f.properties.name,
			// Flat [lng, lat, lng, lat, ...] arrays; holes are rings too (fill with even-odd).
			rings: polygons.flat().map(ring)
		};
	});

const out = new URL('../mobile/shared/world.json', import.meta.url);
writeFileSync(out, JSON.stringify(world));
console.log(`${world.length} countries, ${world.reduce((n, c) => n + c.rings.reduce((m, r) => m + r.length / 2, 0), 0)} points`);
