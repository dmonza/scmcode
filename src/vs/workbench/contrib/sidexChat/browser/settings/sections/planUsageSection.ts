/*---------------------------------------------------------------------------------------------
 *  Usage section — local token/spend totals, per provider and overall.
 *--------------------------------------------------------------------------------------------*/

import type { SettingsSection } from '../sidexSettingsPanel.js';

type TauriInvoke = ((cmd: string, args?: Record<string, unknown>) => Promise<unknown>) | null;

interface UsageAccount {
	id: string;
	label: string;
	source: string;
	inputTokens: number;
	outputTokens: number;
	cost: number;
}

interface TauriUsage {
	totalCost: number;
	totalInputTokens: number;
	totalOutputTokens: number;
	periodStart: string;
	periodEnd: string;
	accounts?: UsageAccount[];
}

export class PlanUsageSection implements SettingsSection {
	constructor(private readonly _invoke: TauriInvoke) {}

	async render(container: HTMLElement): Promise<void> {
		const title = document.createElement('div');
		title.className = 'sidex-settings-section-title no-border';
		title.textContent = 'Usage';
		container.appendChild(title);

		if (!this._invoke) {
			this._renderError(container, 'Tauri not available.');
			return;
		}

		const loading = document.createElement('div');
		loading.style.cssText = 'font-size:12px;padding:8px 24px;color:var(--vscode-descriptionForeground);';
		loading.textContent = 'Loading…';
		container.appendChild(loading);

		let usage: TauriUsage | null = null;
		try {
			usage = await this._invoke('auth_get_usage') as TauriUsage;
		} catch { /* local server may not be up yet */ }

		loading.remove();

		if (!usage) {
			this._renderError(container, 'Usage data is unavailable.');
			return;
		}

		const accounts = Array.isArray(usage.accounts) ? usage.accounts : [];
		for (const account of accounts) {
			this._renderAccount(container, account);
		}
		this._renderTotals(container, usage);
		this._renderFooter(container);
	}

	private _renderAccount(container: HTMLElement, account: UsageAccount): void {
		const card = document.createElement('div');
		card.className = 'sidex-settings-card';
		card.style.marginBottom = '12px';
		container.appendChild(card);

		const head = document.createElement('div');
		head.className = 'sidex-settings-row';
		const titleWrap = document.createElement('div');
		titleWrap.style.minWidth = '0';
		const title = document.createElement('div');
		title.className = 'sidex-settings-row-label';
		title.textContent = account.label;
		titleWrap.appendChild(title);
		const sub = document.createElement('div');
		sub.className = 'sidex-settings-row-description';
		sub.textContent = sourceLabel(account.source);
		titleWrap.appendChild(sub);
		head.appendChild(titleWrap);
		card.appendChild(head);

		const tokensRow = document.createElement('div');
		tokensRow.className = 'sidex-settings-row';
		const tokensLeft = document.createElement('div');
		const tokensLabel = document.createElement('div');
		tokensLabel.className = 'sidex-settings-row-label';
		tokensLabel.textContent = 'Tokens in scmCode';
		tokensLeft.appendChild(tokensLabel);
		const tokensDesc = document.createElement('div');
		tokensDesc.className = 'sidex-settings-row-description';
		tokensDesc.textContent = `${Number(account.inputTokens || 0).toLocaleString()} in / ${Number(account.outputTokens || 0).toLocaleString()} out · ${formatUSD(account.cost || 0)}`;
		tokensLeft.appendChild(tokensDesc);
		tokensRow.appendChild(tokensLeft);
		card.appendChild(tokensRow);
	}

	private _renderTotals(container: HTMLElement, usage: TauriUsage): void {
		const card = document.createElement('div');
		card.className = 'sidex-settings-card';
		container.appendChild(card);

		const tokensRow = document.createElement('div');
		tokensRow.className = 'sidex-settings-row';

		const tokensLeft = document.createElement('div');
		const tokensLabel = document.createElement('div');
		tokensLabel.className = 'sidex-settings-row-label';
		tokensLabel.textContent = 'All providers';
		tokensLeft.appendChild(tokensLabel);

		const tokensDesc = document.createElement('div');
		tokensDesc.className = 'sidex-settings-row-description';
		tokensDesc.textContent = `${Number(usage.totalInputTokens || 0).toLocaleString()} in / ${Number(usage.totalOutputTokens || 0).toLocaleString()} out`;
		tokensLeft.appendChild(tokensDesc);
		tokensRow.appendChild(tokensLeft);

		const tokensVal = document.createElement('div');
		tokensVal.className = 'sidex-settings-row-label';
		tokensVal.style.fontWeight = '600';
		tokensVal.textContent = formatUSD(usage.totalCost ?? 0);
		tokensRow.appendChild(tokensVal);
		card.appendChild(tokensRow);
	}

	private _renderFooter(container: HTMLElement): void {
		const card = document.createElement('div');
		card.className = 'sidex-settings-card';
		container.appendChild(card);

		const actionRow = document.createElement('div');
		actionRow.className = 'sidex-settings-row';

		const actionDesc = document.createElement('div');
		actionDesc.className = 'sidex-settings-row-description';
		actionDesc.style.cssText = 'min-width:0;flex:1;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;';
		actionDesc.textContent = 'Token and spend totals are recorded on this machine by scmCode.';
		actionRow.appendChild(actionDesc);

		const configureBtn = document.createElement('button');
		configureBtn.className = 'sidex-settings-btn';
		configureBtn.style.whiteSpace = 'nowrap';
		configureBtn.style.flexShrink = '0';
		configureBtn.textContent = 'Configure Providers';
		configureBtn.addEventListener('click', () => {
			window.dispatchEvent(new CustomEvent('sidex-settings-navigate', { detail: 'models' }));
		});
		actionRow.appendChild(configureBtn);
		card.appendChild(actionRow);
	}

	private _renderError(container: HTMLElement, msg: string): void {
		const el = document.createElement('div');
		el.style.cssText = 'font-size:12px;padding:8px 24px;color:var(--vscode-descriptionForeground);';
		el.textContent = msg;
		container.appendChild(el);
	}

	dispose(): void {}
}

function sourceLabel(source: string): string {
	switch (source) {
		case 'api_key': return 'API key';
		case 'local': return 'Local server';
		default: return 'Recorded in scmCode';
	}
}

function formatUSD(n: number): string {
	if (!Number.isFinite(n) || n === 0) {
		return '$0.00';
	}
	return `$${n.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`;
}
