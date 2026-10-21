/*---------------------------------------------------------------------------------------------
 *  Sidex — The built-in AI panel for Sidex IDE.
 *  Registered in the AuxiliaryBar with its own status bar toggle.
 *--------------------------------------------------------------------------------------------*/

import * as nls from '../../../../nls.js';
import { Registry } from '../../../../platform/registry/common/platform.js';
import { SyncDescriptor } from '../../../../platform/instantiation/common/descriptors.js';
import { ViewPaneContainer } from '../../../browser/parts/views/viewPaneContainer.js';
import {
	Extensions as ViewExtensions,
	IViewContainersRegistry,
	IViewsRegistry,
	ViewContainerLocation,
} from '../../../common/views.js';
import { registerIcon } from '../../../../platform/theme/common/iconRegistry.js';
import { Codicon } from '../../../../base/common/codicons.js';
import { registerAction2, Action2, MenuId } from '../../../../platform/actions/common/actions.js';
import { KeyMod, KeyCode } from '../../../../base/common/keyCodes.js';
import { KeybindingWeight } from '../../../../platform/keybinding/common/keybindingsRegistry.js';
import { ServicesAccessor } from '../../../../platform/instantiation/common/instantiation.js';
import { IWorkbenchLayoutService, Parts } from '../../../services/layout/browser/layoutService.js';
import { IStatusbarService, StatusbarAlignment } from '../../../services/statusbar/browser/statusbar.js';
import { ContextKeyExpr } from '../../../../platform/contextkey/common/contextkey.js';
import {
	ConfigurationScope,
	Extensions as ConfigurationExtensions,
	IConfigurationRegistry
} from '../../../../platform/configuration/common/configurationRegistry.js';
import { CommandsRegistry } from '../../../../platform/commands/common/commands.js';
import { INotificationService, Severity } from '../../../../platform/notification/common/notification.js';
import { IHostService } from '../../../services/host/browser/host.js';
import { SIDEX_AI_ENABLED_SETTING } from '../../../browser/parts/sidex/sidexPart.js';
import { SidexChatViewPane } from './sidexChatView.js';
import { restartServer } from './localServer.js';
import './sidexChatService.js';

export const SIDEX_CHAT_CONTAINER_ID = 'workbench.view.sidexChat';
export const SIDEX_CHAT_VIEW_ID = 'workbench.view.sidexChat.main';

// --- scmCode: AI on/off ---
// AI is opt-in (`sidex.ai.enabled`, default off). Every AI command and
// keybinding below is gated on it, and the status bar entry, the completion
// provider and the agent server are only set up while it's on.

Registry.as<IConfigurationRegistry>(ConfigurationExtensions.Configuration).registerConfiguration({
	id: 'sidex',
	title: nls.localize('sidexAiConfigurationTitle', 'scmCode AI'),
	type: 'object',
	properties: {
		[SIDEX_AI_ENABLED_SETTING]: {
			type: 'boolean',
			default: false,
			scope: ConfigurationScope.APPLICATION,
			markdownDescription: nls.localize(
				'sidexAiEnabled',
				'Turn on the AI features: the chat panel, inline edit (`Ctrl+K`) and inline completions. While off, the local agent server is not started. Requires a window reload.'
			)
		}
	}
});

/** True while the user has AI turned on. Gates every AI command and keybinding. */
const AiEnabled = ContextKeyExpr.equals(`config.${SIDEX_AI_ENABLED_SETTING}`, true);

// Lets the scmCode settings panel, which lives outside dependency injection,
// flip the setting through the configuration service so the change is
// persisted and the reload prompt below fires.
CommandsRegistry.registerCommand('sidex.ai.setEnabled', async (accessor, enabled: unknown) => {
	if (typeof enabled === 'boolean') {
		await accessor.get(IConfigurationService).updateValue(SIDEX_AI_ENABLED_SETTING, enabled);
	}
});

const sidexChatIcon = registerIcon('sidex-chat-icon', Codicon.commentDiscussion, nls.localize('sidexChatIcon', 'scmCode icon'));

const viewContainer = Registry.as<IViewContainersRegistry>(ViewExtensions.ViewContainersRegistry).registerViewContainer(
	{
		id: SIDEX_CHAT_CONTAINER_ID,
		title: nls.localize2('sidex', 'scmCode'),
		icon: sidexChatIcon,
		ctorDescriptor: new SyncDescriptor(ViewPaneContainer, [SIDEX_CHAT_CONTAINER_ID, { mergeViewWithContainerWhenSingleView: true }]),
		hideIfEmpty: false,
		order: -100,
	},
	ViewContainerLocation.Sidex,
	{ isDefault: true, doNotRegisterOpenCommand: true }
);

Registry.as<IViewsRegistry>(ViewExtensions.ViewsRegistry).registerViews(
	[
		{
			id: SIDEX_CHAT_VIEW_ID,
			name: nls.localize2('sidex', 'scmCode'),
			containerIcon: sidexChatIcon,
			ctorDescriptor: new SyncDescriptor(SidexChatViewPane),
			canToggleVisibility: false,
			canMoveView: false,
			hideByDefault: false,
		},
	],
	viewContainer
);

// Cmd+Shift+I toggles the Sidex panel
registerAction2(class extends Action2 {
	constructor() {
		super({
			id: 'workbench.action.toggleSidexChat',
			title: nls.localize2('toggleSidex', 'Toggle scmCode'),
			precondition: AiEnabled,
			keybinding: {
				primary: KeyMod.CtrlCmd | KeyMod.Shift | KeyCode.KeyI,
				weight: KeybindingWeight.WorkbenchContrib,
				when: AiEnabled,
			},
			menu: { id: MenuId.CommandPalette, when: AiEnabled },
		});
	}
	async run(accessor: ServicesAccessor): Promise<void> {
		const layoutService = accessor.get(IWorkbenchLayoutService);
		layoutService.setPartHidden(layoutService.isVisible(Parts.SIDEX_PART), Parts.SIDEX_PART);
	}
});

// Status bar toggle icon (layout-sidebar-right)
registerAction2(class extends Action2 {
	constructor() {
		super({
			id: 'workbench.action.sidexStatusBarToggle',
			title: nls.localize2('sidexStatusBarToggle', 'Toggle scmCode'),
			precondition: AiEnabled,
		});
	}
	async run(accessor: ServicesAccessor): Promise<void> {
		const layoutService = accessor.get(IWorkbenchLayoutService);
		const isVisible = layoutService.isVisible(Parts.SIDEX_PART);
		layoutService.setPartHidden(isVisible, Parts.SIDEX_PART);
	}
});

// Register the status bar entries on startup
import { IWorkbenchContribution, registerWorkbenchContribution2, WorkbenchPhase } from '../../../common/contributions.js';
import { Disposable, IDisposable } from '../../../../base/common/lifecycle.js';
import { IConfigurationService } from '../../../../platform/configuration/common/configuration.js';
import { ICodeEditorService } from '../../../../editor/browser/services/codeEditorService.js';
import { InlineEditController } from './inline/inlineEditController.js';
import { isCodeEditor } from '../../../../editor/browser/editorBrowser.js';
import { EditorContextKeys } from '../../../../editor/common/editorContextKeys.js';

class SidexStatusBarContribution implements IWorkbenchContribution {
	static readonly ID = 'sidex.statusbar';

	constructor(
		@IStatusbarService private readonly statusbarService: IStatusbarService,
		@IWorkbenchLayoutService private readonly layoutService: IWorkbenchLayoutService,
		@IConfigurationService configurationService: IConfigurationService,
	) {
		// Sidex panel toggle (far right). Only while AI is on: with it off
		// there is no panel to toggle.
		const sidexEntry = configurationService.getValue<boolean>(SIDEX_AI_ENABLED_SETTING)
			? statusbarService.addEntry(
					{
						name: nls.localize('sidexToggle', 'scmCode'),
						text: this._sidexIcon(),
						ariaLabel: nls.localize('toggleSidex', 'Toggle scmCode'),
						command: 'workbench.action.sidexStatusBarToggle',
						tooltip: this._sidexTooltip()
					},
					'sidex.toggle',
					StatusbarAlignment.RIGHT,
					-1000
				)
			: undefined;

		// Secondary sidebar toggle (next to Sidex, for Claude Code etc.)
		const auxEntry = statusbarService.addEntry(
			{
				name: nls.localize('auxToggle', 'Secondary Sidebar'),
				text: this._auxIcon(),
				ariaLabel: nls.localize('toggleAux', 'Toggle Secondary Sidebar'),
				command: 'workbench.action.toggleAuxiliaryBar',
				tooltip: this._auxTooltip(),
			},
			'sidex.aux.toggle',
			StatusbarAlignment.RIGHT,
			-999
		);

		// Update icons when visibility changes
		const update = () => {
			sidexEntry?.update({
				name: nls.localize('sidexToggle', 'scmCode'),
				text: this._sidexIcon(),
				ariaLabel: nls.localize('toggleSidex', 'Toggle scmCode'),
				command: 'workbench.action.sidexStatusBarToggle',
				tooltip: this._sidexTooltip(),
			});
			auxEntry.update({
				name: nls.localize('auxToggle', 'Secondary Sidebar'),
				text: this._auxIcon(),
				ariaLabel: nls.localize('toggleAux', 'Toggle Secondary Sidebar'),
				command: 'workbench.action.toggleAuxiliaryBar',
				tooltip: this._auxTooltip(),
			});
		};

		// Listen for layout changes
		(layoutService as any).onDidChangePartVisibility?.(() => update());
		// Fallback: poll every 500ms (some layout changes don't fire events)
		const interval = setInterval(update, 500);
		(this as unknown as { dispose?: IDisposable }).dispose = { dispose: () => clearInterval(interval) } as IDisposable;
	}

	private _sidexIcon(): string {
		return this.layoutService.isVisible(Parts.SIDEX_PART)
			? '$(sidex-panel-open)'
			: '$(sidex-panel-closed)';
	}

	private _sidexTooltip(): string {
		return this.layoutService.isVisible(Parts.SIDEX_PART) ? 'Hide scmCode' : 'Show scmCode';
	}

	private _auxIcon(): string {
		return this.layoutService.isVisible(Parts.AUXILIARYBAR_PART)
			? '$(layout-sidebar-right)'
			: '$(layout-sidebar-right-off)';
	}

	private _auxTooltip(): string {
		return this.layoutService.isVisible(Parts.AUXILIARYBAR_PART) ? 'Hide Secondary Sidebar' : 'Show Secondary Sidebar';
	}
}

registerWorkbenchContribution2(SidexStatusBarContribution.ID, SidexStatusBarContribution, WorkbenchPhase.AfterRestored);

/**
 * The AI setting is read once at startup (panel, status bar entry, completion
 * provider, and the agent server on the Rust side), so a change only takes
 * effect after a reload. Reloading the window doesn't restart the app process,
 * so the agent server is started or stopped explicitly first.
 */
class SidexAiToggleContribution extends Disposable implements IWorkbenchContribution {
	static readonly ID = 'sidex.aiToggle';

	constructor(
		@IConfigurationService configurationService: IConfigurationService,
		@INotificationService notificationService: INotificationService,
		@IHostService hostService: IHostService,
		@IWorkbenchLayoutService layoutService: IWorkbenchLayoutService
	) {
		super();
		this._register(
			configurationService.onDidChangeConfiguration(e => {
				if (!e.affectsConfiguration(SIDEX_AI_ENABLED_SETTING)) {
					return;
				}
				const enabled = configurationService.getValue<boolean>(SIDEX_AI_ENABLED_SETTING) === true;
				notificationService.prompt(
					Severity.Info,
					enabled
						? nls.localize('sidexAiOnReload', 'AI features will be turned on after a reload.')
						: nls.localize('sidexAiOffReload', 'AI features will be turned off after a reload.'),
					[
						{
							label: nls.localize('reloadWindow', 'Reload Window'),
							run: async () => {
								await restartServer();
								if (enabled) {
									// Show the panel on reload instead of leaving the user
									// to find the toggle.
									layoutService.setPartHidden(false, Parts.SIDEX_PART);
								}
								await hostService.reload();
							}
						}
					]
				);
			})
		);
	}
}

registerWorkbenchContribution2(SidexAiToggleContribution.ID, SidexAiToggleContribution, WorkbenchPhase.AfterRestored);

// Old Haiku-based inline completion — disabled in favor of Sidex Complete (sidexComplete contribution)
// class SidexInlineCompletionContribution implements IWorkbenchContribution {
// 	static readonly ID = 'sidex.inlineCompletion';
// }
// registerWorkbenchContribution2(SidexInlineCompletionContribution.ID, SidexInlineCompletionContribution, WorkbenchPhase.AfterRestored);

// --- CMD+K Inline Edit ---

// One InlineEditController per editor, lazily created
const controllerMap = new WeakMap<object, InlineEditController>();

registerAction2(class extends Action2 {
	constructor() {
		super({
			id: 'sidex.inlineEdit.activate',
			title: nls.localize2('sidexInlineEdit', 'scmCode: Inline Edit'),
			precondition: AiEnabled,
			keybinding: {
				primary: KeyMod.CtrlCmd | KeyCode.KeyK,
				weight: KeybindingWeight.EditorContrib + 100,
				// Gated on AI so that, while it's off, Ctrl+K falls through to
				// the editor's own Ctrl+K chords (comment line, fold, …).
				when: ContextKeyExpr.and(AiEnabled, EditorContextKeys.editorTextFocus),
			},
			menu: { id: MenuId.CommandPalette, when: AiEnabled },
		});
	}
	async run(accessor: ServicesAccessor): Promise<void> {
		const codeEditorService = accessor.get(ICodeEditorService);
		const configService = accessor.get(IConfigurationService);
		const editor = codeEditorService.getFocusedCodeEditor();
		if (!editor || !isCodeEditor(editor)) {
			return;
		}

		let controller = controllerMap.get(editor);
		if (!controller) {
			controller = new InlineEditController(editor, configService);
			controllerMap.set(editor, controller);
			// Clean up when editor is disposed
			editor.onDidDispose(() => {
				controller?.dispose();
				controllerMap.delete(editor);
			});
		}

		controller.activate();
	}
});
