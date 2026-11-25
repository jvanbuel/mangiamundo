// Builds the app icon with a real outline of the continents:
//   web/static/icon.svg and the Android launcher foreground.
// Run from the repository root after `npm install` in web/:  node tools/build-logo.mjs bite
// The iOS icon is a 1024px PNG render of the SVG with square corners.
// `node tools/build-logo.mjs loader` writes the animated startup loader into web/src/app.html,
// the Android splash icon and the iOS launch screen globe.
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { globe } from './logo/globe.mjs';
import { variants } from './logo/icon.mjs';
import { androidForeground, androidSplash } from './logo/android.mjs';
import { loader } from './logo/loader.mjs';

export { land } from './logo/globe.mjs';
export { variants } from './logo/icon.mjs';
export { loader } from './logo/loader.mjs';

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
