// Runs before each test file (vite.config.ts).
import { beforeEach } from 'vitest';

// jsdom has no matchMedia, which Svelte's `prefersReducedMotion` reads. The tests prefer reduced
// motion, so a spin lands as soon as the server answers instead of turning the globe first.
Object.defineProperty(window, 'matchMedia', {
	value: (query: string) => ({
		matches: query.includes('prefers-reduced-motion'),
		media: query,
		addEventListener() {},
		removeEventListener() {}
	})
});

beforeEach(() => {
	localStorage.clear();
	history.replaceState(null, '', '/');
	// A request no test answered fails as if the server were down, instead of reaching out.
	globalThis.fetch = () => Promise.reject(new TypeError('No server in the tests'));
});
