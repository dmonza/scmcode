/*---------------------------------------------------------------------------------------------
 *  scmCode: URLs of the web workers in a built app, by worker label (docs/issues/closed/ISSUE-0016).
 *
 *  `?worker&url` makes Vite bundle each worker on its own, with everything it imports. SideX
 *  listed the workers as extra inputs of the main build instead, so Rollup put the code they
 *  share with the window into common chunks that read `window` at load, and the workers died
 *  with "window is not defined" (or never loaded at all on Windows). Upstream builds each
 *  worker as its own bundle too.
 *--------------------------------------------------------------------------------------------*/

import editorWorkerUrl from '../../../editor/common/services/editorWebWorkerMain.ts?worker&url';
import textMateWorkerUrl from '../../../workbench/services/textMate/browser/backgroundTokenization/worker/textMateTokenizationWorker.workerMain.ts?worker&url';
import outputLinkWorkerUrl from '../../../workbench/contrib/output/common/outputLinkComputerMain.ts?worker&url';
import extensionHostWorkerUrl from '../../../workbench/api/worker/extensionHostWorkerMain.ts?worker&url';

export const SCMCODE_WORKER_URLS: Readonly<Record<string, string>> = {
	editorWorkerService: editorWorkerUrl,
	TextMateWorker: textMateWorkerUrl,
	OutputLinkDetectionWorker: outputLinkWorkerUrl,
	extensionHostWorkerMain: extensionHostWorkerUrl
};
