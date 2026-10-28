<script lang="ts">
	// Income and spending per month as grouped columns, one currency per chart. The table below holds every value,
	// for whoever cannot see or hover the chart.
	import type { Schemas } from '#lib/api/client.ts';
	import { compact, money, month, monthShort } from '#lib/format.ts';
	import { t } from '#lib/i18n.svelte.ts';

	let {
		rows,
		months,
		currency
	}: { rows: Schemas['MonthTotals'][]; months: string[]; currency: string } = $props();

	const id = $props.id();
	const HEIGHT = 240;
	const TOP = 12;
	const BOTTOM = 40;
	const LEFT = 56;
	const RIGHT = 8;
	const TICKS = 4;
	const PLOT = HEIGHT - TOP - BOTTOM;

	let width = $state(0);
	let active = $state<number | null>(null);

	// Every month of the period, with zeros where no money moved: a missing month must not shorten the axis.
	const points = $derived(
		months.map((m) => {
			const row = rows.find((r) => r.month === m);
			return { month: m, income: row?.income ?? 0, spending: row?.spending ?? 0 };
		})
	);
	const plotWidth = $derived(Math.max(0, width - LEFT - RIGHT));
	const band = $derived(points.length ? plotWidth / points.length : 0);
	const barWidth = $derived(Math.max(2, Math.min(24, (band * 0.8 - 2) / 2)));
	const step = $derived(
		niceStep(Math.max(0, ...points.flatMap((p) => [p.income, p.spending])) / TICKS)
	);
	const top = $derived(step * TICKS);
	// A month label every `every` months, so labels stay about 44 px apart.
	const every = $derived(Math.max(1, Math.ceil(44 / Math.max(band, 1))));
	const title = $derived(`${t('chart.title')} · ${currency}`);
	const shown = $derived(active === null ? null : points[active]);

	/** The smallest of 1, 2, 2.5 or 5 times a power of ten that is at least `raw`: clean tick values. */
	function niceStep(raw: number): number {
		if (raw <= 0) return 100;
		const power = 10 ** Math.floor(Math.log10(raw));
		return ([1, 2, 2.5, 5, 10].find((f) => f * power >= raw) ?? 10) * power;
	}

	const y = (value: number) => TOP + PLOT * (1 - value / top);

	/** A column from the baseline up, its top corners rounded 4 px and its foot square. */
	function column(x: number, value: number): string {
		const height = PLOT * (value / top);
		if (height <= 0) return '';
		const r = Math.min(4, height, barWidth / 2);
		const base = TOP + PLOT;
		const head = base - height;
		return (
			`M${x},${base}V${head + r}A${r},${r} 0 0 1 ${x + r},${head}` +
			`H${x + barWidth - r}A${r},${r} 0 0 1 ${x + barWidth},${head + r}V${base}Z`
		);
	}

	const left = (index: number) => LEFT + band * index + (band - (barWidth * 2 + 2)) / 2;

	function point(event: PointerEvent) {
		const box = (event.currentTarget as SVGElement).getBoundingClientRect();
		const index = Math.floor((event.clientX - box.left - LEFT) / band);
		active = index >= 0 && index < points.length ? index : null;
	}

	function key(event: KeyboardEvent) {
		const last = points.length - 1;
		const moves: Record<string, number> = {
			ArrowLeft: Math.max(0, (active ?? last + 1) - 1),
			ArrowRight: Math.min(last, (active ?? -1) + 1),
			Home: 0,
			End: last
		};
		if (event.key in moves) {
			event.preventDefault();
			active = moves[event.key];
		}
	}

	const tipLeft = $derived(active === null ? 0 : LEFT + band * (active + 0.5));
	// Arrow keys move through the months like a slider; a screen reader reads the month's values as its value.
	const current = $derived(active ?? Math.max(0, points.length - 1));
	const spoken = $derived.by(() => {
		const p = points[current];
		if (!p) return '';
		return [
			month(p.month),
			`${t('tiles.income')} ${money(p.income, currency)}`,
			`${t('tiles.spending')} ${money(p.spending, currency)}`,
			`${t('chart.net')} ${money(p.income - p.spending, currency)}`
		].join(', ');
	});
</script>

<figure class="chart" aria-labelledby="{id}-title">
	<figcaption id="{id}-title">{title}</figcaption>
	<ul class="legend">
		<li><span class="swatch income"></span>{t('tiles.income')}</li>
		<li><span class="swatch spending"></span>{t('tiles.spending')}</li>
	</ul>
	<div
		class="plot"
		bind:clientWidth={width}
		role="slider"
		tabindex="0"
		aria-label={title}
		aria-orientation="horizontal"
		aria-valuemin={0}
		aria-valuemax={Math.max(0, points.length - 1)}
		aria-valuenow={current}
		aria-valuetext={spoken}
		onkeydown={key}
		onfocus={() => (active ??= points.length - 1)}
		onblur={() => (active = null)}
	>
		{#if width > 0}
			<svg
				{width}
				height={HEIGHT}
				aria-hidden="true"
				onpointermove={point}
				onpointerleave={() => (active = null)}
			>
				{#if active !== null}
					<rect class="hover" x={LEFT + band * active} y={TOP} width={band} height={PLOT} />
				{/if}
				{#each { length: TICKS + 1 }, i (i)}
					<line
						class={i === 0 ? 'axis' : 'grid'}
						x1={LEFT}
						x2={LEFT + plotWidth}
						y1={y(step * i)}
						y2={y(step * i)}
					/>
					<text class="tick" x={LEFT - 6} y={y(step * i)} dy="0.32em" text-anchor="end">
						{compact(step * i, currency)}
					</text>
				{/each}
				{#each points as p, i (p.month)}
					<path class="income" d={column(left(i), p.income)} />
					<path class="spending" d={column(left(i) + barWidth + 2, p.spending)} />
					{#if i % every === 0}
						<text class="tick" x={LEFT + band * (i + 0.5)} y={TOP + PLOT + 16} text-anchor="middle">
							{monthShort(p.month)}
						</text>
						{#if i === 0 || p.month.endsWith('-01')}
							<text
								class="tick"
								x={LEFT + band * (i + 0.5)}
								y={TOP + PLOT + 32}
								text-anchor="middle"
							>
								{p.month.slice(0, 4)}
							</text>
						{/if}
					{/if}
				{/each}
			</svg>
		{/if}
		{#if shown}
			<div
				class="tip"
				style:left="{Math.min(Math.max(tipLeft, 90), Math.max(width - 90, 90))}px"
				aria-hidden="true"
			>
				<strong>{month(shown.month)}</strong>
				<span
					><span class="key income"></span><b>{money(shown.income, currency)}</b>
					{t('tiles.income')}</span
				>
				<span
					><span class="key spending"></span><b>{money(shown.spending, currency)}</b>
					{t('tiles.spending')}</span
				>
				<span><b>{money(shown.income - shown.spending, currency)}</b> {t('chart.net')}</span>
			</div>
		{/if}
	</div>
	<details>
		<summary>{t('chart.table')}</summary>
		<div class="table-wrap">
			<table>
				<caption class="visually-hidden">{title}</caption>
				<thead>
					<tr>
						<th scope="col">{t('chart.month')}</th>
						<th scope="col" class="number">{t('tiles.income')}</th>
						<th scope="col" class="number">{t('tiles.spending')}</th>
						<th scope="col" class="number">{t('chart.net')}</th>
					</tr>
				</thead>
				<tbody>
					{#each points as p (p.month)}
						<tr>
							<th scope="row">{month(p.month)}</th>
							<td class="number">{money(p.income, currency)}</td>
							<td class="number">{money(p.spending, currency)}</td>
							<td class="number">{money(p.income - p.spending, currency)}</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	</details>
</figure>

<style>
	/* One column that may shrink below its content: a wide table scrolls in its own box, and the plot measures
	   the card's width, not the table's. */
	.chart {
		margin: 0;
		display: grid;
		grid-template-columns: minmax(0, 1fr);
		gap: 8px;
	}
	figcaption {
		font-weight: 650;
	}
	.legend {
		display: flex;
		flex-wrap: wrap;
		gap: 4px 16px;
		margin: 0;
		padding: 0;
		list-style: none;
		color: var(--muted);
		font-size: 0.9rem;
	}
	.legend li {
		display: flex;
		align-items: center;
		gap: 6px;
	}
	.swatch {
		width: 12px;
		height: 12px;
		border-radius: 3px;
	}
	.swatch.income,
	.key.income {
		background: var(--chart-income);
	}
	.swatch.spending,
	.key.spending {
		background: var(--chart-spending);
	}
	.plot {
		position: relative;
		min-height: 240px;
		border-radius: var(--radius);
	}
	svg {
		display: block;
		touch-action: pan-y;
	}
	path.income {
		fill: var(--chart-income);
	}
	path.spending {
		fill: var(--chart-spending);
	}
	.grid {
		stroke: var(--chart-grid);
		stroke-width: 1;
	}
	.axis {
		stroke: var(--chart-axis);
		stroke-width: 1;
	}
	.hover {
		fill: var(--bg);
	}
	.tick {
		fill: var(--muted);
		font-size: 12px;
		font-variant-numeric: tabular-nums;
	}
	.tip {
		position: absolute;
		top: 0;
		transform: translateX(-50%);
		display: grid;
		gap: 2px;
		min-width: 170px;
		padding: 8px 10px;
		background: var(--surface);
		border: 1px solid var(--border);
		border-radius: var(--radius);
		box-shadow: 0 2px 8px rgb(0 0 0 / 0.15);
		font-size: 0.9rem;
		pointer-events: none;
	}
	.tip span {
		display: flex;
		align-items: center;
		gap: 6px;
		color: var(--muted);
	}
	.tip b {
		color: var(--text);
		font-variant-numeric: tabular-nums;
	}
	.key {
		width: 12px;
		height: 2px;
	}
	summary {
		cursor: pointer;
		padding: 8px 0;
		min-height: 44px;
	}
	th[scope='row'] {
		font-weight: 400;
		text-align: start;
	}
</style>
