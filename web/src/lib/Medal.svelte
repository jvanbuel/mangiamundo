<script lang="ts">
	import BadgeCheck from '@lucide/svelte/icons/badge-check';
	import Crown from '@lucide/svelte/icons/crown';
	import Flame from '@lucide/svelte/icons/flame';
	import Flag from '@lucide/svelte/icons/flag';
	import Globe from '@lucide/svelte/icons/globe';
	import Map from '@lucide/svelte/icons/map';
	import MapPinPlus from '@lucide/svelte/icons/map-pin-plus';
	import PartyPopper from '@lucide/svelte/icons/party-popper';
	import Target from '@lucide/svelte/icons/target';
	import Utensils from '@lucide/svelte/icons/utensils';
	import type { BadgeKind } from './types';

	/** Badge kinds, plus `event` for time-limited badges and `quest` for monthly quests. */
	type MedalKind = BadgeKind | 'event' | 'quest';

	let { kind, earned = true, size = 64 }: { kind: MedalKind; earned?: boolean; size?: number } = $props();

	const ICONS = {
		visits: Utensils,
		countries: Flag,
		completed: BadgeCheck,
		continent: Map,
		world: Globe,
		everything: Crown,
		pioneer: MapPinPlus,
		streak: Flame,
		event: PartyPopper,
		quest: Target
	};
	/** Gold for tasting, diamond for finishing, blue for the rest. */
	const FACES: Record<MedalKind, 'gold' | 'diamond' | 'blue' | 'event'> = {
		visits: 'gold',
		countries: 'gold',
		completed: 'diamond',
		continent: 'diamond',
		world: 'diamond',
		everything: 'diamond',
		pioneer: 'blue',
		streak: 'gold',
		event: 'event',
		quest: 'blue'
	};
	const COLORS = { gold: 'var(--discovered)', diamond: 'var(--diamond)', blue: 'var(--accent)', event: 'var(--event)' };
	const EDGES = {
		gold: 'var(--discovered-edge)',
		diamond: 'var(--completed-edge)',
		blue: 'color-mix(in srgb, var(--accent) 70%, #000)',
		event: 'color-mix(in srgb, var(--event) 70%, #000)'
	};
	const INKS = { gold: 'var(--on-discovered)', diamond: 'var(--on-completed)', blue: 'var(--on-accent)', event: 'var(--on-event)' };
	const face = $derived(FACES[kind]);
	const Icon = $derived(ICONS[kind]);
</script>

<span
	class="medal"
	class:shine={earned && face === 'diamond'}
	style:--size="{size}px"
	style:--color={earned ? COLORS[face] : 'var(--line)'}
	style:--edge={earned ? EDGES[face] : 'color-mix(in srgb, var(--line) 70%, #000)'}
	style:--ink={earned ? INKS[face] : 'var(--muted)'}
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
		box-shadow: inset 0 calc(var(--size) * -0.07) 0 var(--edge);
	}

	/* Diamond medals catch the light now and then: a glint sweeps across. */
	.shine {
		position: relative;
		overflow: hidden;
	}

	.shine::after {
		content: '';
		position: absolute;
		inset: 0;
		background: linear-gradient(115deg, transparent 35%, rgb(255 255 255 / 0.9) 50%, transparent 65%);
		transform: translateX(-120%);
		animation: glint 4s ease-in-out infinite;
	}

	.shine :global(svg) {
		position: relative;
		z-index: 1;
	}

	@keyframes glint {
		0%, 70% { transform: translateX(-120%); }
		100% { transform: translateX(120%); }
	}
</style>
