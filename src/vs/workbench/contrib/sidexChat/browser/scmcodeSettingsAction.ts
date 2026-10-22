/*---------------------------------------------------------------------------------------------
 *  scmCode: "scmCode Settings" (AI on/off, model providers) in the Manage (gear) menu and
 *  the Command Palette. It used to live in the title bar profile button, which was removed
 *  so the title bar matches VS Code. Extensions add their own entries to the same menu
 *  through MenuId.GlobalActivity.
 *--------------------------------------------------------------------------------------------*/

import { localize2 } from '../../../../nls.js';
import { Action2, MenuId, registerAction2 } from '../../../../platform/actions/common/actions.js';
import { SidexSettingsPanel } from './settings/sidexSettingsPanel.js';

registerAction2(
	class extends Action2 {
		constructor() {
			super({
				id: 'scmcode.openSettings',
				title: localize2('scmcodeOpenSettings', 'scmCode Settings'),
				f1: true,
				menu: { id: MenuId.GlobalActivity, group: '2_configuration', order: 3 }
			});
		}

		run(): void {
			SidexSettingsPanel.getInstance().open();
		}
	}
);
