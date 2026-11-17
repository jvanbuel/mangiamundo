import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import { execSync } from 'node:child_process';
import { defineConfig } from 'vite';

/** The commit shown in the app. Docker builds have no .git and pass GIT_COMMIT instead. */
function commit(): string | undefined {
	if (process.env.GIT_COMMIT) return process.env.GIT_COMMIT;
	try {
		return execSync('git describe --always --dirty', { stdio: ['ignore', 'pipe', 'ignore'] }).toString().trim();
	} catch {
		return undefined; // SvelteKit then uses the build time
	}
}

export default defineConfig({
	plugins: [
		sveltekit({
			compilerOptions: {
				// Force runes mode for the project, except for libraries. Can be removed in svelte 6.
				runes: ({ filename }) =>
					filename.split(/[/\\]/).includes('node_modules') ? undefined : true
			},
			// A client-rendered single-page app; the Rust backend serves the build output.
			adapter: adapter({ fallback: 'index.html' }),
			version: { name: commit() }
		})
	],
	server: {
		proxy: { '/api': 'http://localhost:8080' }
	}
});
