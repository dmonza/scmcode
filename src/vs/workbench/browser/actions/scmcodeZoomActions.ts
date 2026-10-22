/*---------------------------------------------------------------------------------------------
 *  Copyright (c) Microsoft Corporation. All rights reserved.
 *  Licensed under the MIT License. See License.txt in the project root for license information.
 *--------------------------------------------------------------------------------------------*/

/*
 * scmCode: window zoom (View > Appearance > Zoom In / Zoom Out / Reset Zoom, the
 * `window.zoomLevel` and `window.zoomPerWindow` settings). VS Code registers these in its
 * Electron layer, which this port does not ship: the actions in
 * electron-browser/actions/windowActions.ts, the settings in
 * electron-browser/desktop.contribution.ts, applying the configured level in
 * electron-browser/window.ts and `applyZoom` in platform/window/electron-browser/window.ts.
 * They are ported here with the Tauri webview zoom in place of Electron's `webFrame`.
 * Each Tauri window is its own webview, so "all windows" is reached through the setting,
 * which every window applies when it changes.
 */

import { localize, localize2 } from '../../../nls.js';
import { getZoomLevel, setZoomFactor, setZoomLevel } from '../../../base/browser/browser.js';
import { mainWindow } from '../../../base/browser/window.js';
import { KeyCode, KeyMod } from '../../../base/common/keyCodes.js';
import { Disposable } from '../../../base/common/lifecycle.js';
import { Categories } from '../../../platform/action/common/actionCommonCategories.js';
import { Action2, MenuId, registerAction2 } from '../../../platform/actions/common/actions.js';
import { IConfigurationService } from '../../../platform/configuration/common/configuration.js';
import {
	Extensions as ConfigurationExtensions,
	IConfigurationRegistry
} from '../../../platform/configuration/common/configurationRegistry.js';
import { ServicesAccessor } from '../../../platform/instantiation/common/instantiation.js';
import { KeybindingWeight } from '../../../platform/keybinding/common/keybindingsRegistry.js';
import { Registry } from '../../../platform/registry/common/platform.js';
import { zoomLevelToZoomFactor } from '../../../platform/window/common/window.js';
import { IWorkbenchContribution, WorkbenchPhase, registerWorkbenchContribution2 } from '../../common/contributions.js';

const MAX_ZOOM_LEVEL = 8;
const MIN_ZOOM_LEVEL = -8;

const ZOOM_LEVEL_SETTING_KEY = 'window.zoomLevel';
const ZOOM_PER_WINDOW_SETTING_KEY = 'window.zoomPerWindow';

Registry.as<IConfigurationRegistry>(ConfigurationExtensions.Configuration).registerConfiguration({
	id: 'window',
	properties: {
		[ZOOM_LEVEL_SETTING_KEY]: {
			type: 'number',
			default: 0,
			minimum: MIN_ZOOM_LEVEL,
			maximum: MAX_ZOOM_LEVEL,
			markdownDescription: localize(
				{ comment: ['{0} will be a setting name rendered as a link'], key: 'zoomLevel' },
				"Adjust the default zoom level for all windows. Each increment above `0` (e.g. `1`) or below (e.g. `-1`) represents zooming `20%` larger or smaller. You can also enter decimals to adjust the zoom level with a finer granularity. See {0} for configuring if the 'Zoom In' and 'Zoom Out' commands apply the zoom level to all windows or only the active window.",
				'`#window.zoomPerWindow#`'
			),
			ignoreSync: true,
			tags: ['accessibility']
		},
		[ZOOM_PER_WINDOW_SETTING_KEY]: {
			type: 'boolean',
			default: true,
			markdownDescription: localize(
				{ comment: ['{0} will be a setting name rendered as a link'], key: 'zoomPerWindow' },
				"Controls if the 'Zoom In' and 'Zoom Out' commands apply the zoom level to all windows or only the active window. See {0} for configuring a default zoom level for all windows.",
				'`#window.zoomLevel#`'
			),
			tags: ['accessibility']
		}
	}
});

/** Applies a zoom level to this window: the Tauri webview and the workbench's zoom state. */
function applyZoom(zoomLevel: number): void {
	zoomLevel = Math.min(Math.max(zoomLevel, MIN_ZOOM_LEVEL), MAX_ZOOM_LEVEL);
	const zoomFactor = zoomLevelToZoomFactor(zoomLevel);

	import('@tauri-apps/api/webview').then(
		({ getCurrentWebview }) => getCurrentWebview().setZoom(zoomFactor),
		() => {
			/* not running in Tauri */
		}
	);
	setZoomFactor(zoomFactor, mainWindow);
	setZoomLevel(zoomLevel, mainWindow);
}

function getConfiguredZoomLevel(configurationService: IConfigurationService): number {
	const windowZoomLevel = configurationService.getValue(ZOOM_LEVEL_SETTING_KEY);
	return typeof windowZoomLevel === 'number' ? windowZoomLevel : 0;
}

abstract class BaseZoomAction extends Action2 {
	protected async setZoomLevel(accessor: ServicesAccessor, levelOrReset: number | true): Promise<void> {
		const configurationService = accessor.get(IConfigurationService);
		const allWindows = configurationService.getValue(ZOOM_PER_WINDOW_SETTING_KEY) === false;

		let level: number;
		if (typeof levelOrReset === 'number') {
			level = Math.round(levelOrReset); // prevent fractional zoom levels
		} else {
			// reset to 0 when we apply to all windows, otherwise to the default zoom level
			level = allWindows ? 0 : getConfiguredZoomLevel(configurationService);
		}

		if (level > MAX_ZOOM_LEVEL || level < MIN_ZOOM_LEVEL) {
			return; // https://github.com/microsoft/vscode/issues/48357
		}

		if (allWindows) {
			await configurationService.updateValue(ZOOM_LEVEL_SETTING_KEY, level);
		}

		applyZoom(level);
	}
}

registerAction2(
	class ZoomInAction extends BaseZoomAction {
		constructor() {
			super({
				id: 'workbench.action.zoomIn',
				title: {
					...localize2('zoomIn', 'Zoom In'),
					mnemonicTitle: localize({ key: 'miZoomIn', comment: ['&& denotes a mnemonic'] }, '&&Zoom In')
				},
				category: Categories.View,
				f1: true,
				keybinding: {
					weight: KeybindingWeight.WorkbenchContrib,
					primary: KeyMod.CtrlCmd | KeyCode.Equal,
					secondary: [KeyMod.CtrlCmd | KeyMod.Shift | KeyCode.Equal, KeyMod.CtrlCmd | KeyCode.NumpadAdd]
				},
				menu: {
					id: MenuId.MenubarAppearanceMenu,
					group: '5_zoom',
					order: 1
				}
			});
		}

		override run(accessor: ServicesAccessor): Promise<void> {
			return super.setZoomLevel(accessor, getZoomLevel(mainWindow) + 1);
		}
	}
);

registerAction2(
	class ZoomOutAction extends BaseZoomAction {
		constructor() {
			super({
				id: 'workbench.action.zoomOut',
				title: {
					...localize2('zoomOut', 'Zoom Out'),
					mnemonicTitle: localize({ key: 'miZoomOut', comment: ['&& denotes a mnemonic'] }, '&&Zoom Out')
				},
				category: Categories.View,
				f1: true,
				keybinding: {
					weight: KeybindingWeight.WorkbenchContrib,
					primary: KeyMod.CtrlCmd | KeyCode.Minus,
					secondary: [KeyMod.CtrlCmd | KeyMod.Shift | KeyCode.Minus, KeyMod.CtrlCmd | KeyCode.NumpadSubtract],
					linux: {
						primary: KeyMod.CtrlCmd | KeyCode.Minus,
						secondary: [KeyMod.CtrlCmd | KeyCode.NumpadSubtract]
					}
				},
				menu: {
					id: MenuId.MenubarAppearanceMenu,
					group: '5_zoom',
					order: 2
				}
			});
		}

		override run(accessor: ServicesAccessor): Promise<void> {
			return super.setZoomLevel(accessor, getZoomLevel(mainWindow) - 1);
		}
	}
);

registerAction2(
	class ZoomResetAction extends BaseZoomAction {
		constructor() {
			super({
				id: 'workbench.action.zoomReset',
				title: {
					...localize2('zoomReset', 'Reset Zoom'),
					mnemonicTitle: localize({ key: 'miZoomReset', comment: ['&& denotes a mnemonic'] }, '&&Reset Zoom')
				},
				category: Categories.View,
				f1: true,
				keybinding: {
					weight: KeybindingWeight.WorkbenchContrib,
					primary: KeyMod.CtrlCmd | KeyCode.Numpad0
				},
				menu: {
					id: MenuId.MenubarAppearanceMenu,
					group: '5_zoom',
					order: 3
				}
			});
		}

		override run(accessor: ServicesAccessor): Promise<void> {
			return super.setZoomLevel(accessor, true);
		}
	}
);

/** Applies `window.zoomLevel` on startup and whenever it changes, as VS Code's NativeWindow does. */
class ScmcodeWindowZoomContribution extends Disposable implements IWorkbenchContribution {
	static readonly ID = 'workbench.contrib.scmcodeWindowZoom';

	constructor(@IConfigurationService private readonly configurationService: IConfigurationService) {
		super();

		applyZoom(getConfiguredZoomLevel(configurationService));

		this._register(
			configurationService.onDidChangeConfiguration(e => {
				if (
					e.affectsConfiguration(ZOOM_LEVEL_SETTING_KEY) ||
					(e.affectsConfiguration(ZOOM_PER_WINDOW_SETTING_KEY) &&
						this.configurationService.getValue(ZOOM_PER_WINDOW_SETTING_KEY) === false)
				) {
					const configuredZoomLevel = getConfiguredZoomLevel(this.configurationService);
					if (getZoomLevel(mainWindow) !== configuredZoomLevel) {
						applyZoom(configuredZoomLevel);
					}
				}
			})
		);
	}
}

registerWorkbenchContribution2(
	ScmcodeWindowZoomContribution.ID,
	ScmcodeWindowZoomContribution,
	WorkbenchPhase.BlockRestore
);
