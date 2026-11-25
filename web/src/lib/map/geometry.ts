import type { Geometry } from 'geojson';

/** Zoom at which the globe's diameter fills most of the map's shorter side. */
export function fittingZoom(el: HTMLElement, lat: number, fill = 0.95): number {
	const diameter = Math.min(el.clientWidth, el.clientHeight) * fill;
	// MapLibre draws the globe with radius 512 * 2^zoom / (2π cos(lat)).
	return Math.log2((diameter * Math.PI * Math.cos((lat * Math.PI) / 180)) / 512);
}

/**
 * The atlas stores countries that cross the antimeridian (Russia, Fiji, ...) as one
 * ring whose longitude jumps from +180 to -180. A flat renderer would draw that jump
 * as a stripe across the whole world, so make each ring continuous instead, letting
 * it run past 180 degrees. MapLibre wraps such geometry correctly.
 */
export function unwrap(geometry: Geometry): Geometry {
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
export function bbox(geometry: Geometry): [number, number, number, number] {
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
export function centre(geometry: Geometry): [number, number] {
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
