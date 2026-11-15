<script lang="ts">
	import '../app.css';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();
</script>

<svelte:head>
	<title>Mangiamundo</title>
	<meta name="description" content="Eat your way around the world without leaving your city." />
	<link rel="icon" href="/icon.svg" type="image/svg+xml" />
	<link rel="apple-touch-icon" href="/icon.svg" />
</svelte:head>

{@render children()}
