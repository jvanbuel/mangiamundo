<script lang="ts">
	import '../app.css';
	import favicon from '#lib/assets/favicon.svg';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();
</script>

<svelte:head>
	<title>Mangiamundo</title>
	<meta name="description" content="Eat your way around the world without leaving your city." />
	<meta name="theme-color" content="#0b1220" />
	<link rel="icon" href={favicon} />
</svelte:head>

{@render children()}
