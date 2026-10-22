/*---------------------------------------------------------------------------------------------
 *  scmCode: "Check for Updates..." in the Manage (gear) menu.
 *
 *  VS Code adds it from contrib/update (`appendUpdateMenuItems(MenuId.GlobalActivity,
 *  '7_update')` in update.ts), which this port does not ship. The check only runs when the
 *  user picks the entry, so it adds no background network call; see PRIVACY.md.
 *--------------------------------------------------------------------------------------------*/

import { localize2 } from '../../../../nls.js';
import { Action2, MenuId, registerAction2 } from '../../../../platform/actions/common/actions.js';
import { ServicesAccessor } from '../../../../platform/instantiation/common/instantiation.js';
import { IUpdateService } from '../../../../platform/update/common/update.js';

registerAction2(
	class extends Action2 {
		constructor() {
			super({
				id: 'update.check',
				title: localize2('checkForUpdates', 'Check for Updates...'),
				f1: true,
				menu: { id: MenuId.GlobalActivity, group: '7_update' }
			});
		}

		run(accessor: ServicesAccessor): Promise<void> {
			return accessor.get(IUpdateService).checkForUpdates(true);
		}
	}
);
