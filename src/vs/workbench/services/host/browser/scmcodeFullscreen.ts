/*---------------------------------------------------------------------------------------------
 *  scmCode: full screen state of the Tauri window.
 *
 *  The web workbench guesses full screen from the window size (`detectFullscreen` in
 *  base/browser/dom.ts): a window as tall as the screen counts as full screen. A maximized
 *  window on a monitor without a taskbar is that tall too, so the guess hides the menu bar
 *  and the window controls. VS Code desktop never guesses: it applies the state the main
 *  process reports (`setFullscreen(!!this.configuration.fullscreen)` in
 *  electron-browser/desktop.main.ts, then `nativeHostService.onDidChangeWindowFullScreen`
 *  in services/host/electron-browser/nativeHostService.ts). Under Tauri we ask the native
 *  window the same way.
 *--------------------------------------------------------------------------------------------*/

export function isTauriWindow(): boolean {
	return !!(globalThis as { __SIDEX_TAURI__?: unknown }).__SIDEX_TAURI__;
}

/** Whether the Tauri window is in full screen, or `undefined` if it can't be asked. */
export async function getTauriFullscreen(): Promise<boolean | undefined> {
	try {
		const { getCurrentWindow } = await import('@tauri-apps/api/window');
		return await getCurrentWindow().isFullscreen();
	} catch {
		return undefined;
	}
}
