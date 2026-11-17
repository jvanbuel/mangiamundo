// Builds the app icon with a real outline of the continents:
//   web/static/icon.svg and the Android launcher foreground.
// Run from the repository root after `npm install` in web/:  node tools/build-logo.mjs bite
// The iOS icon is a 1024px PNG render of the SVG with square corners.
// `node tools/build-logo.mjs loader` writes the animated startup loader into web/src/app.html,
// the Android splash icon and the iOS launch screen globe.
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
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

// Loader timing in ms: whole globe, then each bite grows, starting STEP apart.
// Kept in step with StartupLoader.kt (Android) and StartupLoader.swift (iOS).
const START = 200, GROW = 150, STEP = 270;

// The bite: three circles in the background colour. Shared by the icon and the loader.
const BITES = [
	[398, 118, 62],
	[318, 92, 46],
	[436, 196, 44]
];

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
		svg('#17324D', [
			globe(256, 262, 176, '#3B8FD9', '#54B96F'),
			`<g fill="#17324D">${BITES.map(([cx, cy, r]) => `<circle cx="${cx}" cy="${cy}" r="${r}"/>`).join('')}</g>`
		].join('\n'))
};

// Android adaptive icon foreground: the same artwork without the background, scaled
// into the safe zone. Only handles the elements the variants above use.
function androidPaths(icon) {
	const attr = (tag, name) => tag.match(new RegExp(` ${name}="([^"]*)"`))?.[1];
	const circle = (cx, cy, r) => `M${cx - r},${cy}a${r},${r} 0 1,0 ${2 * r},0a${r},${r} 0 1,0 ${-2 * r},0z`;
	const rect = (x, y, w, h, rx) =>
		`M${x + rx},${y}h${w - 2 * rx}a${rx},${rx} 0 0 1 ${rx},${rx}v${h - 2 * rx}a${rx},${rx} 0 0 1 ${-rx},${rx}h${-(w - 2 * rx)}a${rx},${rx} 0 0 1 ${-rx},${-rx}v${-(h - 2 * rx)}a${rx},${rx} 0 0 1 ${rx},${-rx}z`;
	let fill = null;
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
		paths.push({ paint, d });
	}
	return paths;
}

const pathTag = ({ paint, d }, indent = '        ') => `${indent}<path ${paint} android:pathData="${d}" />`;

function androidForeground(icon) {
	const paths = androidPaths(icon).map((p) => pathTag(p));
	return `<?xml version="1.0" encoding="utf-8"?>
<!-- Generated by tools/build-logo.mjs: the app icon without its background, in the adaptive-icon safe zone. -->
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

// Android splash icon (API 31+ animates it): the bite icon without its square, bites in
// the splash background colour so they match the light or dark splash screen. Same size
// and safe zone as the launcher foreground, which is what the splash screen expects.
function androidSplash(icon) {
	const [ocean, land, ...bites] = androidPaths(icon);
	const biteColor = 'android:fillColor="@color/splash_background"';
	const header = (comment) => `<?xml version="1.0" encoding="utf-8"?>
<!-- Generated by tools/build-logo.mjs: ${comment} -->`;
	const vector = (body, indent = '') =>
		`${indent}<vector xmlns:android="http://schemas.android.com/apk/res/android"
${indent}    android:width="108dp"
${indent}    android:height="108dp"
${indent}    android:viewportWidth="108"
${indent}    android:viewportHeight="108">
${indent}    <group android:scaleX="0.18" android:scaleY="0.18" android:translateX="7.92" android:translateY="7.92">
${body}
${indent}    </group>
${indent}</vector>`;
	const still = header('the splash icon with its bites, shown without animation before API 31.') + '\n' +
		vector([ocean, land, ...bites.map((b) => ({ ...b, paint: biteColor }))].map((p) => pathTag(p)).join('\n')) + '\n';

	const i8 = '                ';
	const body = [
		`${i8}<group android:name="globe" android:pivotX="256" android:pivotY="262">`,
		pathTag(ocean, i8 + '    '),
		pathTag(land, i8 + '    '),
		`${i8}</group>`,
		...BITES.flatMap(([cx, cy], i) => [
			`${i8}<group android:name="bite${i}" android:pivotX="${cx}" android:pivotY="${cy}" android:scaleX="0" android:scaleY="0">`,
			pathTag({ ...bites[i], paint: biteColor }, i8 + '    '),
			`${i8}</group>`
		])
	].join('\n');
	const animator = (prop, from, to, offset, duration, interpolator) =>
		`                <objectAnimator android:propertyName="${prop}" android:valueType="floatType" android:valueFrom="${from}" android:valueTo="${to}" android:startOffset="${offset}" android:duration="${duration}" android:interpolator="${interpolator}" />`;
	const target = (name, animators) => `    <target android:name="${name}">
        <aapt:attr name="android:animation">
            <set android:ordering="together">
${animators.join('\n')}
            </set>
        </aapt:attr>
    </target>`;
	const overshoot = '@android:interpolator/overshoot';
	const ease = '@android:interpolator/fast_out_slow_in';
	// A small squash of the globe on each bite, as on the web.
	const squash = BITES.flatMap((_, i) => {
		const at = START + i * STEP;
		return [
			animator('scaleX', 1, 0.96, at, 77, ease), animator('scaleX', 0.96, 1, at + 77, 143, ease),
			animator('scaleY', 1, 1.03, at, 77, ease), animator('scaleY', 1.03, 1, at + 77, 143, ease)
		];
	});
	const animated = `${header('the splash icon whose bites are taken out one by one (API 31+).')}
<animated-vector xmlns:android="http://schemas.android.com/apk/res/android"
    xmlns:aapt="http://schemas.android.com/aapt">
    <aapt:attr name="android:drawable">
${vector(body, '        ')}
    </aapt:attr>
${target('globe', squash)}
${BITES.map((_, i) => target(`bite${i}`, ['scaleX', 'scaleY'].map((prop) => animator(prop, 0, 1, START + i * STEP, GROW, overshoot)))).join('\n')}
</animated-vector>
`;
	return { still, animated };
}

// Startup loader for web/src/app.html: the bite icon without its square, on the page
// background, with the three bites growing in one after another. Plain CSS so it plays
// before any JavaScript loads; src/lib/splash.ts takes it away once the first data is in.
// Timing: whole globe for 200 ms, then each bite grows in 150 ms, starting 270 ms apart.
export function loader() {
	const [start, grow, step] = [START, GROW, STEP];
	const end = start + 2 * step + grow;
	// One nested group per bite, so the three squashes don't fight over one transform.
	const squash = (inner, _bite, i) =>
		`<g class="squash" style="animation-delay:${start + i * step}ms">${inner}</g>`;
	const globeSvg = BITES.reduce(squash, globe(256, 262, 176, '#3B8FD9', '#54B96F'));
	const bites = BITES.map(
		([cx, cy, r], i) =>
			`<circle class="bite" style="animation-delay:${start + i * step}ms" cx="${cx}" cy="${cy}" r="${r}"/>`
	).join('');
	return `<!-- splash:start (generated by tools/build-logo.mjs loader, do not edit by hand) -->
		<style>
			#splash {
				--splash-bg: #f2f5f8;
				position: fixed;
				inset: 0;
				z-index: 1000;
				display: grid;
				place-items: center;
				background: var(--splash-bg);
				transition: opacity 250ms ease-out;
			}
			@media (prefers-color-scheme: dark) {
				:root:not([data-theme='light']) #splash { --splash-bg: #0d1a28; }
			}
			:root[data-theme='dark'] #splash { --splash-bg: #0d1a28; }
			#splash[data-state='gone'] { opacity: 0; pointer-events: none; }
			#splash svg { width: min(40vmin, 180px); height: auto; overflow: visible; }
			#splash .bites { fill: var(--splash-bg); }
			#splash .bite, #splash .squash, #splash .pulse { transform-box: fill-box; transform-origin: center; }
			#splash .bite { animation: mm-bite ${grow}ms cubic-bezier(0.2, 0.9, 0.3, 1.2) both; }
			#splash .squash { animation: mm-squash 220ms ease-out both; }
			#splash .pulse { animation: mm-pulse 1.6s ease-in-out ${end + 300}ms infinite; }
			@keyframes mm-bite { from { transform: scale(0); } to { transform: scale(1); } }
			@keyframes mm-squash {
				0%, 100% { transform: none; }
				35% { transform: scale(0.96, 1.03); }
				70% { transform: scale(1.01, 0.99); }
			}
			@keyframes mm-pulse { 0%, 100% { transform: none; } 50% { transform: scale(1.03); } }
			@media (prefers-reduced-motion: reduce) {
				#splash, #splash * { animation: none !important; transition: none !important; }
			}
		</style>
		<div id="splash" role="img" aria-label="Mangiamundo is loading">
			<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" aria-hidden="true">
				<g class="pulse">${globeSvg}<g class="bites">${bites}</g></g>
			</svg>
		</div>
		<!-- splash:end -->`;
}

// Only write the files when run directly, not when imported.
const name = process.argv[1] === new URL(import.meta.url).pathname ? process.argv[2] : null;
if (name === 'loader') {
	const file = new URL('../web/src/app.html', import.meta.url);
	const html = readFileSync(file, 'utf8');
	const marked = /<!-- splash:start[\s\S]*?<!-- splash:end -->/;
	if (!marked.test(html)) throw new Error('web/src/app.html has no <!-- splash:start --> ... <!-- splash:end --> markers');
	writeFileSync(file, html.replace(marked, () => loader()));
	const { still, animated } = androidSplash(variants.bite());
	const res = '../mobile/android/app/src/main/res/';
	mkdirSync(new URL(res + 'drawable-v31/', import.meta.url), { recursive: true });
	writeFileSync(new URL(res + 'drawable/splash_icon.xml', import.meta.url), still);
	writeFileSync(new URL(res + 'drawable-v31/splash_icon.xml', import.meta.url), animated);
	// iOS: the launch screen is static, so it shows the whole globe; StartupLoaderView then
	// takes the bites at the same size and place. 180 pt square, as on the web.
	writeFileSync(
		new URL('../mobile/ios/Mangiamundo/Assets.xcassets/LaunchGlobe.imageset/launch-globe.svg', import.meta.url),
		`<svg xmlns="http://www.w3.org/2000/svg" width="180" height="180" viewBox="0 0 512 512">\n${globe(256, 262, 176, '#3B8FD9', '#54B96F')}\n</svg>\n`
	);
	console.log('wrote the startup loader into web/src/app.html, the Android splash icon and the iOS launch globe');
} else if (name) {
	if (!variants[name]) throw new Error(`unknown variant ${name}; try ${Object.keys(variants).join(', ')}`);
	const icon = variants[name]();
	const android = '../mobile/android/app/src/main/res/';
	writeFileSync(new URL('../web/static/icon.svg', import.meta.url), icon);
	writeFileSync(new URL(android + 'drawable/ic_launcher_foreground.xml', import.meta.url), androidForeground(icon));
	writeFileSync(
		new URL(android + 'values/colors.xml', import.meta.url),
		`<resources>\n    <color name="ic_launcher_background">${icon.match(/rx="112" fill="([^"]*)"/)[1]}</color>\n</resources>\n`
	);
	console.log(`wrote the ${name} icon for web and Android`);
}
