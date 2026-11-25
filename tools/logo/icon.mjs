// The app icon's artwork: its variants, and the bite they share with the loaders.
import { globe } from './globe.mjs';

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

const svg = (background, body) =>
	`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" role="img" aria-label="Mangiamundo">\n<rect width="512" height="512" rx="112" fill="${background}"/>\n${body}\n</svg>\n`;

// Loader timing in ms: whole globe, then each bite grows, starting STEP apart.
// Kept in step with StartupLoader.kt (Android) and StartupLoader.swift (iOS).
export const START = 200, GROW = 150, STEP = 270;

// The bite: three circles in the background colour. Shared by the icon and the loader.
export const BITES = [
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
