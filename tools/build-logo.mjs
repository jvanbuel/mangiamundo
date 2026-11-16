// Builds the app icon with a real outline of the continents:
//   web/static/icon.svg and the Android launcher foreground.
// The bite variant also writes the startup loader artwork: the globe before the bites
// (web/static/globe.svg, the iOS LaunchGlobe asset) and the Android splash vector.
// Run from the repository root after `npm install` in web/:  node tools/build-logo.mjs bite
// The iOS icon is a 1024px PNG render of the SVG with square corners.
import { writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';

const require = createRequire(new URL('../web/package.json', import.meta.url));
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

const fork = (x, top, height, color, scale = 1) => {
	const w = 17 * scale, gap = 12 * scale, head = 4 * w + 3 * gap, left = x - head / 2;
	const tine = 0.3 * height, bowl = head / 2;
	const tines = [0, 1, 2, 3]
		.map((i) => `<rect x="${left + i * (w + gap)}" y="${top}" width="${w}" height="${tine}" rx="${w / 2}"/>`)
		.join('');
	return `<g fill="${color}">${tines}<path d="M${left} ${top + tine - 20}h${head}v${12}a${bowl} ${bowl} 0 0 1 ${-head} 0z"/><rect x="${x - 13 * scale}" y="${top + tine + bowl * 0.5}" width="${26 * scale}" height="${height - tine - bowl * 0.5}" rx="${13 * scale}"/></g>`;
};

const knife = (x, top, height, color, scale = 1) => {
	const w = 46 * scale, blade = 0.56 * height;
	return `<g fill="${color}"><path d="M${x - w / 2} ${top + blade}V${top + w}a${w} ${w} 0 0 1 ${w} ${-w}V${top + blade}z"/><rect x="${x - 13 * scale}" y="${top + blade - 8}" width="${26 * scale}" height="${height - blade + 8}" rx="${13 * scale}"/></g>`;
};

const globe = (cx, cy, r, ocean, green, lng0 = 12, lat0 = 22) =>
	`<circle cx="${cx}" cy="${cy}" r="${r}" fill="${ocean}"/><path d="${land(cx, cy, r, lng0, lat0)}" fill="${green}"/>`;

const svg = (background, body) =>
	`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" role="img" aria-label="Mangiamundo">\n<rect width="512" height="512" rx="112" fill="${background}"/>\n${body}\n</svg>\n`;

/** The bite: circles [cx, cy, r] in the background colour, in the order the loader bites. */
export const BITES = [[398, 118, 62], [318, 92, 46], [436, 196, 44]];
const bites = () => `<g fill="#17324D">${BITES.map(([cx, cy, r]) => `<circle cx="${cx}" cy="${cy}" r="${r}"/>`).join('')}</g>`;
const biteGlobe = () => globe(256, 262, 176, '#3B8FD9', '#54B96F');

export const variants = {
	// The world served on a plate, between fork and knife.
	plate: () =>
		svg('#17324D', [
			fork(74, 116, 280, '#F6EFE3', 0.8),
			knife(440, 116, 280, '#F6EFE3', 0.8),
			'<circle cx="256" cy="256" r="150" fill="#F6EFE3"/>',
			'<circle cx="256" cy="256" r="150" fill="none" stroke="#D5CCBB" stroke-width="5"/>',
			globe(256, 256, 116, '#3B8FD9', '#54B96F')
		].join('\n')),
	// The world with a bite taken out of it.
	bite: () =>
		svg('#17324D', [biteGlobe(), bites()].join('\n'))
};

// Android adaptive icon foreground: the same artwork without the background, scaled
// into the safe zone. Only handles the elements the variants above use.
// With `splash`, each bite circle sits in a group (bite0, bite1, ...) scaled to 0, for
// the animated splash icon (res/drawable/splash_animated.xml) to grow.
function androidForeground(icon, splash = false) {
	const attr = (tag, name) => tag.match(new RegExp(` ${name}="([^"]*)"`))?.[1];
	const circle = (cx, cy, r) => `M${cx - r},${cy}a${r},${r} 0 1,0 ${2 * r},0a${r},${r} 0 1,0 ${-2 * r},0z`;
	const rect = (x, y, w, h, rx) =>
		`M${x + rx},${y}h${w - 2 * rx}a${rx},${rx} 0 0 1 ${rx},${rx}v${h - 2 * rx}a${rx},${rx} 0 0 1 ${-rx},${rx}h${-(w - 2 * rx)}a${rx},${rx} 0 0 1 ${-rx},${-rx}v${-(h - 2 * rx)}a${rx},${rx} 0 0 1 ${rx},${-rx}z`;
	let fill = null, bite = 0;
	const paths = [];
	for (const tag of icon.match(/<(g|circle|rect|path)[^>]*>/g).slice(1)) {
		const n = (name) => Number(attr(tag, name));
		if (tag.startsWith('<g')) fill = attr(tag, 'fill');
		const color = attr(tag, 'fill') ?? fill;
		let d;
		if (tag.startsWith('<circle')) d = circle(n('cx'), n('cy'), n('r'));
		else if (tag.startsWith('<rect')) d = rect(n('x'), n('y'), n('width'), n('height'), n('rx'));
		else if (tag.startsWith('<path')) d = attr(tag, 'd');
		else continue;
		const paint = color === 'none'
			? `android:strokeColor="${attr(tag, 'stroke')}" android:strokeWidth="${attr(tag, 'stroke-width')}"`
			: `android:fillColor="${color}"`;
		// Trim floating-point noise such as 6.800000000000001.
		d = d.replace(/-?\d+\.\d{3,}/g, (number) => String(Math.round(Number(number) * 100) / 100));
		const path = `<path ${paint} android:pathData="${d}" />`;
		paths.push(splash && fill && tag.startsWith('<circle')
			? `        <group android:name="bite${bite++}" android:pivotX="${n('cx')}" android:pivotY="${n('cy')}" android:scaleX="0" android:scaleY="0">${path}</group>`
			: `        ${path}`);
	}
	return `<?xml version="1.0" encoding="utf-8"?>
<!-- Generated by tools/build-logo.mjs: the app icon without its background, in the adaptive-icon safe zone${splash ? ', bites scaled to 0' : ''}. -->
<vector xmlns:android="http://schemas.android.com/apk/res/android"
    android:width="108dp"
    android:height="108dp"
    android:viewportWidth="108"
    android:viewportHeight="108">
    <group android:scaleX="0.18" android:scaleY="0.18" android:translateX="7.92" android:translateY="7.92">
${paths.join('\n')}
    </group>
</vector>
`;
}

// Only write the files when run directly, not when imported.
const name = process.argv[1] === new URL(import.meta.url).pathname ? process.argv[2] : null;
if (name) {
	if (!variants[name]) throw new Error(`unknown variant ${name}; try ${Object.keys(variants).join(', ')}`);
	const icon = variants[name]();
	const android = '../mobile/android/app/src/main/res/';
	writeFileSync(new URL('../web/static/icon.svg', import.meta.url), icon);
	writeFileSync(new URL(android + 'drawable/ic_launcher_foreground.xml', import.meta.url), androidForeground(icon));
	writeFileSync(
		new URL(android + 'values/colors.xml', import.meta.url),
		`<resources>\n    <color name="ic_launcher_background">${icon.match(/rx="112" fill="([^"]*)"/)[1]}</color>\n</resources>\n`
	);
	if (name === 'bite') {
		// 200 x 200 points is the size the iOS launch screen shows it at; the loader views match it.
		const whole = svg('#17324D', biteGlobe()).replace('<svg ', '<svg width="200" height="200" ');
		writeFileSync(new URL('../web/static/globe.svg', import.meta.url), whole);
		writeFileSync(new URL('../mobile/ios/Mangiamundo/Assets.xcassets/LaunchGlobe.imageset/globe.svg', import.meta.url), whole);
		writeFileSync(new URL(android + 'drawable/splash_globe.xml', import.meta.url), androidForeground(icon, true));
	}
	console.log(`wrote the ${name} icon for web and Android`);
}
