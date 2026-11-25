<script lang="ts">
	import LocationPicker from './LocationPicker.svelte';
</script>

<div class="card welcome">
	<h2>Which of the world's kitchens are near you?</h2>
	<p class="muted">
		Tell us where you are and we will light up every country whose kitchen you can visit nearby.
	</p>
	<LocationPicker />
	<p class="muted about">
		Check in at restaurants to taste each country's food and earn levels and badges. No account
		needed: your progress stays on this device. Signing in with Google, Apple or email is optional
		and only keeps your progress on all your devices. <a href="/privacy/">Privacy policy</a>
	</p>
</div>

<style>
	.about {
		margin: 12px 0 0;
		font-size: var(--text-xs);
	}

	/* Panels sit on a tinted surface. The heavy lower edge is kept for things you can press. */
	.card {
		padding: 16px;
		border-radius: 14px;
		background: var(--panel-raised);
		transition: opacity 0.2s;
	}

	.card h2 {
		font-size: var(--text-lg);
		margin-bottom: 6px;
	}

	.card p {
		margin: 0 0 12px;
	}

	.welcome h2 {
		font-size: var(--text-xl);
		line-height: 1.25;
	}
</style>
