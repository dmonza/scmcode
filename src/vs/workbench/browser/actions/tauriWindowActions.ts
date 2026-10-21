/*---------------------------------------------------------------------------------------------
 *  Tauri window actions for scmCode
 *  "Close Window" and "Exit" are registered by the Electron (desktop) layer of VS Code,
 *  which this Tauri port does not ship, so they are provided here on top of the Tauri
 *  window API instead.
 *--------------------------------------------------------------------------------------------*/

import { localize, localize2 } from '../../../nls.js';
import { MenuRegistry, MenuId, Action2, registerAction2 } from '../../../platform/actions/common/actions.js';
import { KeyCode, KeyMod } from '../../../base/common/keyCodes.js';
import { IsMacContext } from '../../../platform/contextkey/common/contextkeys.js';
import { KeybindingWeight } from '../../../platform/keybinding/common/keybindingsRegistry.js';
import { ServicesAccessor } from '../../../platform/instantiation/common/instantiation.js';
import { ConfirmResult, IFileDialogService } from '../../../platform/dialogs/common/dialogs.js';
import { ILogService } from '../../../platform/log/common/log.js';
import { CancellationToken } from '../../../base/common/cancellation.js';
import { SaveReason } from '../../common/editor.js';
import { ILifecycleService } from '../../services/lifecycle/common/lifecycle.js';
import { IWorkingCopyService } from '../../services/workingCopy/common/workingCopyService.js';
import { IWorkingCopyBackupService } from '../../services/workingCopy/common/workingCopyBackup.js';
import { IFilesConfigurationService } from '../../services/filesConfiguration/common/filesConfigurationService.js';

/**
 * Makes sure no unsaved change is lost before the window goes away: with hot exit the
 * changes are backed up (and restored on the next start), otherwise the user is asked
 * to save them. Resolves to `false` when the user cancels.
 */
async function confirmShutdown(accessor: ServicesAccessor): Promise<boolean> {
	const workingCopyService = accessor.get(IWorkingCopyService);
	const workingCopyBackupService = accessor.get(IWorkingCopyBackupService);
	const filesConfigurationService = accessor.get(IFilesConfigurationService);
	const fileDialogService = accessor.get(IFileDialogService);
	const logService = accessor.get(ILogService);

	const modifiedWorkingCopies = workingCopyService.modifiedWorkingCopies;
	if (!modifiedWorkingCopies.length) {
		return true;
	}

	if (filesConfigurationService.isHotExitEnabled) {
		try {
			await Promise.all(
				modifiedWorkingCopies.map(async workingCopy => {
					const backup = await workingCopy.backup(CancellationToken.None);
					await workingCopyBackupService.backup(workingCopy, backup.content, undefined, backup.meta);
				})
			);

			return true;
		} catch (error) {
			logService.error('[scmCode] Backup before shutdown failed, asking to save instead', error);
		}
	}

	const result = await fileDialogService.showSaveConfirm(modifiedWorkingCopies.map(workingCopy => workingCopy.name));
	if (result === ConfirmResult.CANCEL) {
		return false;
	}

	if (result === ConfirmResult.SAVE) {
		for (const workingCopy of modifiedWorkingCopies) {
			if (!(await workingCopy.save({ reason: SaveReason.EXPLICIT }))) {
				return false; // save was cancelled or failed
			}
		}

		return true;
	}

	// Don't save: drop the backups so the discarded changes do not come back on the next start
	await workingCopyBackupService.discardBackups();

	return true;
}

async function shutdownAndClose(accessor: ServicesAccessor, exitApp: boolean): Promise<void> {
	const lifecycleService = accessor.get(ILifecycleService);

	if (!(await confirmShutdown(accessor))) {
		return;
	}

	await lifecycleService.shutdown();

	const { getCurrentWindow } = await import('@tauri-apps/api/window');
	const { invoke } = await import('@tauri-apps/api/core');
	const appWindow = getCurrentWindow();

	await invoke('save_window_state', { label: appWindow.label }).catch(() => {});

	if (exitApp) {
		await invoke('exit_app');
	} else {
		await appWindow.close();
	}
}

class CloseWindowAction extends Action2 {
	static readonly ID = 'workbench.action.closeWindow';

	constructor() {
		super({
			id: CloseWindowAction.ID,
			title: {
				...localize2('closeWindow', 'Close Window'),
				mnemonicTitle: localize({ key: 'miCloseWindow', comment: ['&& denotes a mnemonic'] }, 'Clos&&e Window')
			},
			f1: true,
			keybinding: {
				weight: KeybindingWeight.WorkbenchContrib,
				mac: { primary: KeyMod.CtrlCmd | KeyMod.Shift | KeyCode.KeyW },
				linux: { primary: KeyMod.Alt | KeyCode.F4, secondary: [KeyMod.CtrlCmd | KeyMod.Shift | KeyCode.KeyW] },
				win: { primary: KeyMod.Alt | KeyCode.F4, secondary: [KeyMod.CtrlCmd | KeyMod.Shift | KeyCode.KeyW] }
			},
			menu: {
				id: MenuId.MenubarFileMenu,
				group: '6_close',
				order: 4
			}
		});
	}

	override run(accessor: ServicesAccessor): Promise<void> {
		return shutdownAndClose(accessor, false);
	}
}

class QuitAction extends Action2 {
	static readonly ID = 'workbench.action.quit';

	constructor() {
		super({
			id: QuitAction.ID,
			title: localize2('quit', 'Exit'),
			f1: true,
			// Like VS Code, no default keybinding on Windows
			keybinding: {
				weight: KeybindingWeight.WorkbenchContrib,
				mac: { primary: KeyMod.CtrlCmd | KeyCode.KeyQ },
				linux: { primary: KeyMod.CtrlCmd | KeyCode.KeyQ }
			}
		});
	}

	override run(accessor: ServicesAccessor): Promise<void> {
		return shutdownAndClose(accessor, true);
	}
}

if ((globalThis as any).__SIDEX_TAURI__) {
	registerAction2(CloseWindowAction);
	registerAction2(QuitAction);

	// On macOS the native application menu already has "Quit"
	MenuRegistry.appendMenuItem(MenuId.MenubarFileMenu, {
		group: 'z_Exit',
		command: {
			id: QuitAction.ID,
			title: localize({ key: 'miExit', comment: ['&& denotes a mnemonic'] }, 'E&&xit')
		},
		order: 1,
		when: IsMacContext.toNegated()
	});
}
