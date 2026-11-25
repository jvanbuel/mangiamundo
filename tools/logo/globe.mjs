// The globe with a real outline of the continents, drawn as SVG.
import { createRequire } from 'node:module';

const require = createRequire(new URL('../../web/package.json', import.meta.url));
const { feature } = require('topojson-client');
const atlas = require('world-atlas/land-110m.json');

const rad = (d) => (d * Math.PI) / 180;

/** SVG path of the land visible on a globe centred on (lng0, lat0), drawn at (cx, cy) with radius r. */
export function land(cx, cy, r, lng0, lat0) {
	const [l0, p0] = [rad(lng0), rad(lat0)];
	const polygons = feature(atlas, atlas.objects.land).features.flatMap((f) =>
		f.geometry.type === 'Polygon' ? [f.geometry.coordinates] : f.geometry.coordinates
	);
	let d = '';
	for (const [outer] of polygons) {
		let visible = 0;
		const points = outer.map(([lng, lat]) => {
			const [l, p] = [rad(lng) - l0, rad(lat)];
			let x = Math.cos(p) * Math.sin(l);
			let y = Math.cos(p0) * Math.sin(p) - Math.sin(p0) * Math.cos(p) * Math.cos(l);
			if (Math.sin(p0) * Math.sin(p) + Math.cos(p0) * Math.cos(p) * Math.cos(l) >= 0) visible++;
			else {
				// Far side: push to the rim so partly hidden continents stay closed.
				const h = Math.hypot(x, y) || 1;
				x /= h;
				y /= h;
			}
			return [cx + x * r, cy - y * r];
		});
		if (visible < 3) continue;
		// Drop specks and thin out points: an icon does not need every fjord.
		const xs = points.map((q) => q[0]), ys = points.map((q) => q[1]);
		if ((Math.max(...xs) - Math.min(...xs)) * (Math.max(...ys) - Math.min(...ys)) < (r * 0.09) ** 2) continue;
		const kept = points.filter((_, i) => i % 2 === 0 || points.length < 24);
		d += 'M' + kept.map(([x, y]) => `${x.toFixed(1)} ${y.toFixed(1)}`).join('L') + 'Z';
	}
	return d;
}

export const globe = (cx, cy, r, ocean, green, lng0 = 12, lat0 = 22) =>
	`<circle cx="${cx}" cy="${cy}" r="${r}" fill="${ocean}"/><path d="${land(cx, cy, r, lng0, lat0)}" fill="${green}"/>`;
