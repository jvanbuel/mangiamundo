<script lang="ts">
	import BadgeCheck from '@lucide/svelte/icons/badge-check';
	import Crown from '@lucide/svelte/icons/crown';
	import Flag from '@lucide/svelte/icons/flag';
	import Globe from '@lucide/svelte/icons/globe';
	import Map from '@lucide/svelte/icons/map';
	import MapPinPlus from '@lucide/svelte/icons/map-pin-plus';
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
		pioneer: MapPinPlus,
		event: PartyPopper
	};
	const COLORS: Record<MedalKind, string> = {
		visits: 'var(--available)',
		countries: 'var(--available)',
		completed: 'var(--completed)',
		continent: 'var(--discovered)',
		world: 'var(--completed)',
		everything: 'var(--discovered)',
		pioneer: 'var(--completed)',
		event: 'var(--event)'
	};
	/** Icon colour on each face: white on deep faces, dark ink on light ones. */
	const INK: Record<MedalKind, string> = {
		visits: 'var(--on-available)',
		countries: 'var(--on-available)',
		completed: 'var(--on-completed)',
		continent: 'var(--on-discovered)',
		world: 'var(--on-completed)',
		everything: 'var(--on-discovered)',
		pioneer: 'var(--on-completed)',
		event: 'var(--on-event)'
	};
	const Icon = $derived(ICONS[kind]);
</script>

<span
	class="medal"
	style:--size="{size}px"
	style:--color={earned ? COLORS[kind] : 'var(--line)'}
	style:--ink={earned ? INK[kind] : 'var(--muted)'}
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
		color: var(--ink);
		box-shadow: inset 0 calc(var(--size) * -0.07) 0 color-mix(in srgb, var(--color) 70%, #000);
	}
</style>
