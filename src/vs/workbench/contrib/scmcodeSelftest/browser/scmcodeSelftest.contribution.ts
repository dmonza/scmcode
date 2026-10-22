/*---------------------------------------------------------------------------------------------
 *  scmCode: runtime self-test of the webview sandbox (src-tauri/src/webview_host.rs).
 *
 *  Inert unless a `selftest` build of the app marks the page (src-tauri/src/selftest.rs,
 *  SCMCODE_SELFTEST=1); CI runs that build on Windows, macOS and Linux. It opens a real
 *  webview, checks from inside that the webview is isolated from the workbench and from
 *  the Tauri IPC, and reports the result to Rust.
 *--------------------------------------------------------------------------------------------*/

import { Disposable } from '../../../../base/common/lifecycle.js';
import { isWindows } from '../../../../base/common/platform.js';
import { dirname } from '../../../../base/common/resources.js';
import { URI } from '../../../../base/common/uri.js';
import { mainWindow } from '../../../../base/browser/window.js';
import { Registry } from '../../../../platform/registry/common/platform.js';
import { IWorkbenchContributionsRegistry, Extensions as WorkbenchExtensions } from '../../../common/contributions.js';
import { LifecyclePhase } from '../../../services/lifecycle/common/lifecycle.js';
import { IWebviewService } from '../../webview/browser/webview.js';
import { asWebviewUri } from '../../webview/common/webview.js';
interface SelftestRequest {
	readonly imagePath: string;
}

/** What the script inside the webview reports. */
interface InsideReport {
	readonly origin: string;
	readonly top: 'readable' | 'blocked';
	readonly ipc: 'allowed' | 'refused' | 'absent';
	readonly asset: 'readable' | 'blocked';
	readonly image: 'loaded' | 'failed';
	// A data: image the page's Content-Security-Policy forbids: 'blocked' proves
	// the restored extension CSP is enforced (TR-03), not stripped as before.
	readonly cspForbiddenImage: 'loaded' | 'blocked';
	readonly serviceWorker: boolean;
}

const START_EVENT = 'scmcode-selftest';
const REPORT_EVENT = 'scmcode-selftest-report';
const WEBVIEW_TIMEOUT_MS = 60_000;

class ScmcodeSelftestContribution extends Disposable {
	private started = false;

	constructor(@IWebviewService private readonly webviewService: IWebviewService) {
		super();
		const request = (mainWindow as any).__SCMCODE_SELFTEST__ as SelftestRequest | undefined;
		if (request) {
			this.run(request);
			return;
		}
		const onStart = (e: Event) => this.run((e as CustomEvent<SelftestRequest>).detail);
		mainWindow.addEventListener(START_EVENT, onStart);
		this._register({ dispose: () => mainWindow.removeEventListener(START_EVENT, onStart) });
	}

	private async run(request: SelftestRequest): Promise<void> {
		if (this.started) {
			return;
		}
		this.started = true;
		let report: Record<string, unknown>;
		try {
			report = await this.check(request);
		} catch (e) {
			report = { ok: false, error: String(e) };
		}
		const { emit } = await import('@tauri-apps/api/event');
		await emit(REPORT_EVENT, report);
	}

	private async check(request: SelftestRequest): Promise<Record<string, unknown>> {
		const image = URI.file(request.imagePath);
		// The sidex-asset URL of a real file; the workbench may read it, a webview may not.
		const assetUrl =
			(isWindows ? 'https://sidex-asset.localhost/' : 'sidex-asset://localhost/') + encodeURIComponent(image.fsPath);
		const workbenchReadsAsset = await fetch(assetUrl).then(
			r => r.ok,
			() => false
		);

		const webview = this._register(
			this.webviewService.createWebviewElement({
				title: 'scmCode self-test',
				options: {},
				contentOptions: { allowScripts: true, localResourceRoots: [dirname(image)] },
				extension: undefined
			})
		);
		const container = mainWindow.document.createElement('div');
		container.style.cssText = 'position:fixed;left:0;top:0;width:200px;height:200px;z-index:100000;';
		mainWindow.document.body.appendChild(container);
		this._register({ dispose: () => container.remove() });
		webview.mountTo(container, mainWindow);

		const inside = new Promise<InsideReport>((resolve, reject) => {
			const timer = setTimeout(() => reject(new Error('the webview did not report in time')), WEBVIEW_TIMEOUT_MS);
			this._register(
				webview.onMessage(e => {
					clearTimeout(timer);
					resolve(e.message as InsideReport);
				})
			);
		});
		webview.setHtml(insideHtml(asWebviewUri(image).toString(true), assetUrl));
		const report = await inside;

		// A second webview with scripts disabled must not run its script
		// (enableScripts honored, TR-03). No message within the window = blocked.
		const noScript = this._register(
			this.webviewService.createWebviewElement({
				title: 'scmCode self-test (no scripts)',
				options: {},
				contentOptions: { allowScripts: false, localResourceRoots: [] },
				extension: undefined
			})
		);
		const noScriptContainer = mainWindow.document.createElement('div');
		noScriptContainer.style.cssText = 'position:fixed;left:0;top:0;width:1px;height:1px;z-index:100000;';
		mainWindow.document.body.appendChild(noScriptContainer);
		this._register({ dispose: () => noScriptContainer.remove() });
		noScript.mountTo(noScriptContainer, mainWindow);
		const scriptRan = await new Promise<boolean>(resolve => {
			const timer = setTimeout(() => resolve(false), 3000);
			this._register(
				noScript.onMessage(() => {
					clearTimeout(timer);
					resolve(true);
				})
			);
			noScript.setHtml(
				'<!DOCTYPE html><html><body><script>try{acquireVsCodeApi().postMessage("ran")}catch(e){}</script></body></html>'
			);
		});

		const checks = {
			sandboxOrigin: /^http:\/\/[0-9a-v]{52}\.localhost:\d+$/.test(report.origin),
			workbenchUnreachable: report.top === 'blocked',
			appCommandsRefused: report.ipc !== 'allowed',
			assetUnreadableFromWebview: report.asset === 'blocked',
			assetReadableFromWorkbench: workbenchReadsAsset,
			localResourceLoads: report.image === 'loaded',
			// With a restrictive CSP present (TR-03), the allowed resource still loads
			// (localResourceLoads above) while a CSP-forbidden one is blocked.
			cspEnforced: report.cspForbiddenImage === 'blocked',
			scriptsBlockedWhenDisabled: !scriptRan
		};
		return { ok: Object.values(checks).every(Boolean), checks, inside: { ...report, scriptRan } };
	}
}

/** The page loaded in the webview: probes from inside and posts the result back. */
function insideHtml(imageSrc: string, assetUrl: string): string {
	// A CSP that restricts images to the webview resource origin (as a real
	// extension's CSP does). It is intentionally narrow to one directive: it must
	// prove the extension CSP is delivered and enforced (TR-03, no longer
	// stripped) without blocking the webview framework's own bootstrap or service
	// worker (a stricter default-src 'none' breaks those). The allowed image must
	// still load through the service worker; a data: image must be blocked.
	const csp = `img-src https://*.vscode-resource.vscode-cdn.net;`;
	// 1x1 transparent GIF; a valid image that loads only if the CSP doesn't block data:.
	const forbidden = 'data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7';
	return `<!DOCTYPE html>
<html>
<head><meta http-equiv="Content-Security-Policy" content="${csp}"></head>
<body>
<img id="img">
<img id="evil">
<script>
(async () => {
	const vscode = acquireVsCodeApi();
	const report = { origin: location.origin };
	try { void window.top.document.body; report.top = 'readable'; } catch { report.top = 'blocked'; }
	const ipc = window.__TAURI_INTERNALS__;
	report.ipc = ipc && ipc.invoke
		? await ipc.invoke('env_app_host').then(() => 'allowed', () => 'refused')
		: 'absent';
	report.asset = await fetch(${JSON.stringify(assetUrl)}).then(r => r.text().then(() => 'readable'), () => 'blocked');
	const img = document.getElementById('img');
	report.image = await new Promise(resolve => {
		img.onload = () => resolve(img.naturalWidth > 0 ? 'loaded' : 'failed');
		img.onerror = () => resolve('failed');
		img.src = ${JSON.stringify(imageSrc)};
	});
	const evil = document.getElementById('evil');
	report.cspForbiddenImage = await new Promise(resolve => {
		evil.onload = () => resolve('loaded');
		evil.onerror = () => resolve('blocked');
		evil.src = ${JSON.stringify(forbidden)};
	});
	report.serviceWorker = !!(navigator.serviceWorker && navigator.serviceWorker.controller);
	vscode.postMessage(report);
})();
</script>
</body></html>`;
}

Registry.as<IWorkbenchContributionsRegistry>(WorkbenchExtensions.Workbench).registerWorkbenchContribution(
	ScmcodeSelftestContribution,
	LifecyclePhase.Restored
);
