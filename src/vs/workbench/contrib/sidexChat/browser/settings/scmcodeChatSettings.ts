/*---------------------------------------------------------------------------------------------
 *  scmCode: the chat settings live in the user settings.json and go through the configuration
 *  service, like every other setting (docs/audit TR-08).
 *
 *  SideX's settings panel and chat views read and wrote them with the Tauri commands
 *  `settings_get` / `settings_update`, backed by a Rust copy of settings.json that was loaded
 *  once at startup and rewrote the whole file on every change. Those commands are gone.
 *  `scmcodeSettingsInvoke` keeps the panel's `invoke(...)` call shape and answers those two
 *  commands from the configuration service, so the panel code stays as upstream wrote it.
 *
 *  The keys are registered because the configuration service, like VS Code's, refuses to
 *  write a key that isn't (`ConfigurationEditing`, ERROR_UNKNOWN_KEY). Defaults are the ones
 *  the panel shows. They are application settings: a workspace can't change them.
 *--------------------------------------------------------------------------------------------*/

import * as nls from '../../../../../nls.js';
import { Disposable } from '../../../../../base/common/lifecycle.js';
import {
	ConfigurationTarget,
	IConfigurationService
} from '../../../../../platform/configuration/common/configuration.js';
import {
	ConfigurationScope,
	Extensions as ConfigurationExtensions,
	IConfigurationPropertySchema,
	IConfigurationRegistry
} from '../../../../../platform/configuration/common/configurationRegistry.js';
import { Registry } from '../../../../../platform/registry/common/platform.js';
import {
	IWorkbenchContribution,
	registerWorkbenchContribution2,
	WorkbenchPhase
} from '../../../../common/contributions.js';

type Invoke = (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;

const panelSetting = nls.localize('scmcodeChatSettingFromPanel', 'Set from scmCode Settings.');
const bool = (value: boolean): IConfigurationPropertySchema => ({
	type: 'boolean',
	default: value,
	description: panelSetting
});
const choice = (value: string, values: string[]): IConfigurationPropertySchema => ({
	type: 'string',
	enum: values,
	default: value,
	description: panelSetting
});

Registry.as<IConfigurationRegistry>(ConfigurationExtensions.Configuration).registerConfiguration({
	id: 'scmcodeChatSettings',
	title: nls.localize('scmcodeChatSettingsTitle', 'scmCode AI Chat'),
	type: 'object',
	scope: ConfigurationScope.APPLICATION,
	properties: {
		'sidex.general.defaultModel': { type: 'string', default: '', description: panelSetting },
		'sidex.general.defaultAgent': choice('agent', ['agent', 'plan', 'ask']),
		'sidex.general.autoScroll': bool(true),
		'sidex.general.windowLayout': choice('Editor', ['Editor', 'Agent']),
		'sidex.general.conversationDensity': choice('Comfortable', ['Compact', 'Comfortable', 'Spacious']),
		'sidex.general.titleBar': bool(true),
		'sidex.general.statusBar': bool(true),
		'sidex.general.reviewControlLocation': choice('Gutter', ['Gutter', 'Toolbar', 'Both']),
		'sidex.general.autoHideEditor': bool(false),
		'sidex.general.chatAsTabs': bool(false),
		'sidex.general.systemNotifications': bool(true),
		'sidex.general.menuBarIcon': bool(true),
		'sidex.general.completionSound': bool(false),
		'sidex.general.dismissedDialogs': { type: 'object', default: {}, description: panelSetting },
		'sidex.models.custom': { type: 'array', default: [], description: panelSetting },
		'sidex.models.enabled': { type: 'array', items: { type: 'string' }, default: [], description: panelSetting },
		'sidex.indexing.autoIndex': bool(true),
		'sidex.indexing.respectGitignore': bool(true),
		'sidex.indexing.instantGrep': bool(true),
		'sidex.rules.includeThirdParty': bool(true),
		'sidex.cascade.background': bool(true),
		'sidex.cascade.autoOpenFiles': bool(true),
		'sidex.cascade.previews': bool(true),
		'sidex.cascade.gitignoreAccess': bool(false),
		'sidex.cascade.autoWebRequestsPolicy': choice('Allowlist', ['Disabled', 'Allowlist', 'Turbo']),
		'sidex.cascade.allowedOrigins': {
			type: 'array',
			items: { type: 'string' },
			default: [],
			description: panelSetting
		},
		'sidex.cascade.allowList': { type: 'array', items: { type: 'string' }, default: [], description: panelSetting },
		'sidex.cascade.denyList': { type: 'array', items: { type: 'string' }, default: [], description: panelSetting },
		'tools.webSearch': bool(true),
		'tools.autoAcceptWebSearch': bool(true),
		'tools.webFetch': bool(true),
		'tools.autoRunMode': choice('Run Everything (Unsandboxed)', [
			'Run Everything (Unsandboxed)',
			'Ask for approval',
			'Sandboxed'
		])
	}
});

/**
 * SideX stored the panel's values as JSON text (`JSON.stringify(value)`), so a settings.json
 * written by an older build holds `"true"` or `"[\"a\"]"`. Such text comes back as the value it
 * encodes; any other string is returned as is.
 */
function decodeStored(value: unknown): unknown {
	if (typeof value !== 'string') {
		return value;
	}
	try {
		return JSON.parse(value);
	} catch {
		return value;
	}
}

/** `decodeStored`, applied to a section object's own properties too (`sidex.general.*`). */
function decodeSection(value: unknown): unknown {
	const decoded = decodeStored(value);
	if (decoded && typeof decoded === 'object' && !Array.isArray(decoded)) {
		const out: Record<string, unknown> = {};
		for (const [key, child] of Object.entries(decoded)) {
			out[key] = decodeStored(child);
		}
		return out;
	}
	return decoded;
}

let configurationService: IConfigurationService | undefined;

/**
 * Wrap a Tauri `invoke` so `settings_get` and `settings_update` go to the configuration
 * service. `settings_get` returns only what the user set (`null` otherwise), as the Rust
 * store did, so the callers' own defaults keep applying. Other commands pass through.
 */
export function scmcodeSettingsInvoke<T extends Invoke | null | undefined>(invoke: T): T {
	if (!invoke) {
		return invoke;
	}
	const wrapped: Invoke = async (cmd, args = {}) => {
		if (cmd === 'settings_get' && configurationService) {
			const section = typeof args.section === 'string' ? args.section : undefined;
			if (!section) {
				return null;
			}
			return decodeSection(configurationService.inspect(section).userValue) ?? null;
		}
		if (cmd === 'settings_update' && configurationService) {
			await configurationService.updateValue(String(args.key), decodeStored(args.value), ConfigurationTarget.USER);
			return null;
		}
		return invoke(cmd, args);
	};
	return wrapped as T;
}

class ScmcodeChatSettingsContribution extends Disposable implements IWorkbenchContribution {
	static readonly ID = 'scmcode.chatSettings';

	constructor(@IConfigurationService service: IConfigurationService) {
		super();
		configurationService = service;
		this._register({ dispose: () => (configurationService = undefined) });
	}
}

registerWorkbenchContribution2(
	ScmcodeChatSettingsContribution.ID,
	ScmcodeChatSettingsContribution,
	WorkbenchPhase.BlockStartup
);
