<script lang="ts">
	import BadgeCheck from '@lucide/svelte/icons/badge-check';
	import Crown from '@lucide/svelte/icons/crown';
	import Flag from '@lucide/svelte/icons/flag';
	import Globe from '@lucide/svelte/icons/globe';
	import Map from '@lucide/svelte/icons/map';
	import PartyPopper from '@lucide/svelte/icons/party-popper';
	import Utensils from '@lucide/svelte/icons/utensils';
	import type { BadgeKind } from './types';

	/** Badge kinds, plus `event` for time-limited badges. */
	type MedalKind = BadgeKind | 'event';

	let { kind, earned = true, size = 64 }: { kind: MedalKind; earned?: boolean; size?: number } = $props();

	const ICONS = {
		visits: Utensils,
		countries: Flag,
		completed: BadgeCheck,
		continent: Map,
		world: Globe,
		everything: Crown,
		event: PartyPopper
	};
	const COLORS: Record<MedalKind, string> = {
		visits: 'var(--available)',
		countries: 'var(--available)',
		completed: 'var(--completed)',
		continent: 'var(--discovered)',
		world: 'var(--completed)',
		everything: 'var(--discovered)',
		event: 'var(--event)'
	};
	const Icon = $derived(ICONS[kind]);
</script>

<span
	class="medal"
	class:locked={!earned}
	style:--size="{size}px"
	style:--color={earned ? COLORS[kind] : 'var(--line)'}
	aria-hidden="true"
>
	<Icon size={size * 0.44} strokeWidth={2.5} />
</span>

<style>
	/* A solid token with a darker lower edge, so it reads as something you collect. */
	.medal {
		width: var(--size);
		height: var(--size);
		flex: none;
		display: grid;
		place-items: center;
		border-radius: 50%;
		background: var(--color);
		color: #fff;
		box-shadow: inset 0 calc(var(--size) * -0.07) 0 color-mix(in srgb, var(--color) 70%, #000);
	}

	.medal.locked {
		color: var(--muted);
	}
</style>
