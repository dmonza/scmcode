/*---------------------------------------------------------------------------------------------
 *  scmCode: a small, dependency-free text encoding guesser for `files.autoGuessEncoding`
 *  (docs/issues/closed/ISSUE-0017).
 *
 *  VS Code guesses with jschardet, which is LGPL-2.1+ and isn't served in the Tauri build.
 *  This guesser covers the same encodings VS Code offers for guessing (`GUESSABLE_ENCODINGS`)
 *  without statistical tables:
 *
 *  1. Pure ASCII → no guess (VS Code never picks ASCII; the default encoding applies).
 *  2. Valid UTF-8 → `utf8`.
 *  3. Otherwise each candidate decodes the bytes, and the decoding that reads least like
 *     mojibake wins: invalid sequences, C1 controls, letters of two alphabets in one word,
 *     impossible letter case, symbols inside words, Japanese without kana, Korean without
 *     Hangul, and alphabets whose most frequent letters are missing all count against it.
 *     Ties go to the earlier candidate in `SCMCODE_GUESS_ENCODINGS`.
 *
 *  Encodings that decode the same letters to the same characters (windows-1255 and
 *  ISO-8859-8, for example) can't be told apart and don't need to be.
 *
 *  It has no imports so it can be tested with plain Node.
 *--------------------------------------------------------------------------------------------*/

/** Decodes `bytes` in `encoding` (an iconv-lite name), replacing invalid sequences with U+FFFD. */
export type ScmcodeDecode = (bytes: Uint8Array, encoding: string) => string;

/** The encodings this guesser chooses between, as VS Code encoding ids, in tie-break order. */
export const SCMCODE_GUESS_ENCODINGS: readonly string[] = [
	'windows1252',
	'windows1250',
	'iso88592',
	'windows1251',
	'koi8r',
	'cp866',
	'cp1125',
	'iso88595',
	'windows1253',
	'iso88597',
	'windows1255',
	'iso88598',
	'shiftjis',
	'eucjp',
	'euckr',
	'gb2312',
	'cp950'
];

const JAPANESE = new Set(['shiftjis', 'eucjp']);
const SCORE_MAX_BYTES = 16 * 1024;

/** The most frequent letters of the main language for each alphabet, lowercase. */
const FREQUENT_LETTERS: Record<string, string> = {
	Cyrl: 'оеаинтсрвлкмдпуіяыьгзбчйжхшюцщэфъё', // і: frequent in Ukrainian
	Grek: 'αοιετσνηυρπκμλωγδχθφβξζψςάέήίόύώ',
	Hebr: 'יוהלמאתרבנשעכדקפחסגטצזךםןףץ'
};
/** How many of the frequent letters above make up most running text. */
const FREQUENT_COUNT: Record<string, number> = { Cyrl: 16, Grek: 16, Hebr: 14 };
/**
 * Share of an alphabet's letters that real text takes from its frequent letters: 0.77-0.83
 * on the test samples, 0.62 for Hebrew read as windows-1251.
 */
const FREQUENT_MIN_SHARE = 0.7;

/**
 * True when `bytes` is UTF-8. A multi-byte sequence cut off at the end of the buffer is
 * accepted, since callers pass only the first bytes of a file, but only after at least one
 * complete multi-byte sequence: a lone trailing byte such as `é` in windows-1252 ("café")
 * is no evidence of UTF-8.
 */
export function isUtf8(bytes: Uint8Array): boolean {
	let sawMultiByte = false;
	let i = 0;
	while (i < bytes.length) {
		const b = bytes[i];
		let extra: number;
		let min: number;
		if (b < 0x80) {
			i++;
			continue;
		} else if (b >= 0xc2 && b <= 0xdf) {
			extra = 1;
			min = 0x80;
		} else if (b >= 0xe0 && b <= 0xef) {
			extra = 2;
			min = 0x800;
		} else if (b >= 0xf0 && b <= 0xf4) {
			extra = 3;
			min = 0x10000;
		} else {
			return false;
		}
		let cp = b & (0x3f >> extra);
		for (let k = 1; k <= extra; k++) {
			if (i + k >= bytes.length) {
				return sawMultiByte; // cut off at the end of the sample
			}
			const c = bytes[i + k];
			if ((c & 0xc0) !== 0x80) {
				return false;
			}
			cp = (cp << 6) | (c & 0x3f);
		}
		if (cp < min || cp > 0x10ffff || (cp >= 0xd800 && cp <= 0xdfff)) {
			return false;
		}
		sawMultiByte = true;
		i += extra + 1;
	}
	return true;
}

/**
 * Guess the encoding of `bytes` (a file's first bytes, without a BOM).
 *
 * @param candidates VS Code encoding ids to choose from (`files.candidateGuessEncodings`),
 * earlier ones winning ties; ids this guesser doesn't know are ignored, and none left means
 * `utf8` and all of `SCMCODE_GUESS_ENCODINGS`.
 * @returns a VS Code encoding id, or `null` when there is nothing to guess (pure ASCII).
 */
export function scmcodeGuessEncoding(
	bytes: Uint8Array,
	decode: ScmcodeDecode,
	candidates?: readonly string[]
): string | null {
	if (!bytes.some(b => b >= 0x80)) {
		return null;
	}
	const wanted = candidates?.filter(c => c === 'utf8' || SCMCODE_GUESS_ENCODINGS.includes(c));
	const pool = wanted && wanted.length > 0 ? wanted : ['utf8', ...SCMCODE_GUESS_ENCODINGS];
	if (pool.includes('utf8') && isUtf8(bytes)) {
		return 'utf8';
	}

	// Scoring decodes the sample once per candidate; 16 KB is plenty to tell them apart and
	// keeps a guess in the low milliseconds (it runs on the UI thread while a file opens).
	const sample = bytes.subarray(0, SCORE_MAX_BYTES);
	let best: string | null = null;
	let bestPenalty = Infinity;
	// The pool's order breaks ties, so `files.candidateGuessEncodings` is tried "in the order
	// listed", as VS Code describes it.
	for (const encoding of pool) {
		if (encoding === 'utf8') {
			continue;
		}
		let text: string;
		try {
			text = decode(sample, encoding);
		} catch {
			continue;
		}
		const penalty = scorePenalty(text, sample, encoding);
		if (penalty < bestPenalty) {
			best = encoding;
			bestPenalty = penalty;
		}
	}
	return best;
}

type Script = 'Latn' | 'Grek' | 'Cyrl' | 'Hebr' | 'Arab' | 'Hang' | 'Hani' | 'Other';

function scriptOf(ch: string): Script {
	const cp = ch.codePointAt(0)!;
	if (cp < 0x0250 || (cp >= 0x1e00 && cp <= 0x1eff)) {
		return 'Latn';
	}
	if (cp >= 0x0370 && cp <= 0x03ff) {
		return 'Grek';
	}
	if (cp >= 0x0400 && cp <= 0x052f) {
		return 'Cyrl';
	}
	if (cp >= 0x0590 && cp <= 0x05ff) {
		return 'Hebr';
	}
	if (cp >= 0x0600 && cp <= 0x06ff) {
		return 'Arab';
	}
	if ((cp >= 0xac00 && cp <= 0xd7af) || (cp >= 0x1100 && cp <= 0x11ff) || (cp >= 0x3130 && cp <= 0x318f)) {
		return 'Hang';
	}
	if (
		(cp >= 0x3040 && cp <= 0x30ff) || // kana
		(cp >= 0x3400 && cp <= 0x4dbf) ||
		(cp >= 0x4e00 && cp <= 0x9fff) ||
		(cp >= 0xf900 && cp <= 0xfaff)
	) {
		return 'Hani';
	}
	return 'Other';
}

const isUpper = (ch: string) => ch !== ch.toLowerCase() && ch === ch.toUpperCase();
const isLower = (ch: string) => ch !== ch.toUpperCase() && ch === ch.toLowerCase();

/** A non-ASCII symbol or punctuation mark, as mojibake puts between letters ("w¹tek": Polish "wątek" read as windows-1252). */
function isStraySymbol(cp: number): boolean {
	return (
		(cp >= 0xa0 && cp <= 0xbf && cp !== 0xad && cp !== 0xb7) || // Latin-1 signs; not soft hyphen, middle dot
		(cp >= 0x2c0 && cp <= 0x2ff) || // spacing modifier letters
		(cp >= 0x2028 && cp <= 0x2bff) // punctuation after the dashes and quotes, symbols, arrows, boxes
	);
}

/** How many stray symbols sit directly between two letters. */
function countSymbolsInWords(chars: string[]): number {
	let count = 0;
	for (let i = 1; i < chars.length - 1; i++) {
		if (isStraySymbol(chars[i].codePointAt(0)!) && isLetter(chars[i - 1]) && isLetter(chars[i + 1])) {
			count++;
		}
	}
	return count;
}

const isLetter = (ch: string) => /\p{L}/u.test(ch);
const WORD = /[\p{L}\p{M}]+/gu;

function scorePenalty(text: string, bytes: Uint8Array, encoding: string): number {
	let penalty = 0;
	let han = 0;
	let kana = 0;
	let hangul = 0;
	const letters: Record<string, number> = {};
	const frequent: Record<string, number> = {};

	for (const ch of text) {
		const cp = ch.codePointAt(0)!;
		if (cp === 0xfffd) {
			penalty += 30;
		} else if (cp >= 0x80 && cp <= 0x9f) {
			penalty += 20; // C1 control
		} else if (cp >= 0xe000 && cp <= 0xf8ff) {
			penalty += 10; // private use
		} else if (cp >= 0xff61 && cp <= 0xff9f) {
			penalty += 4; // half-width katakana: rare in text, common in misread Shift_JIS
		}
		if (cp >= 0x3040 && cp <= 0x30ff) {
			kana++;
		} else if (cp >= 0xac00 && cp <= 0xd7af) {
			hangul++;
		} else if ((cp >= 0x4e00 && cp <= 0x9fff) || (cp >= 0x3400 && cp <= 0x4dbf)) {
			han++;
		}
		const script = scriptOf(ch);
		if (script in FREQUENT_LETTERS && /\p{L}/u.test(ch)) {
			letters[script] = (letters[script] ?? 0) + 1;
			const rank = FREQUENT_LETTERS[script].indexOf(ch.toLowerCase());
			if (rank >= 0 && rank < FREQUENT_COUNT[script]) {
				frequent[script] = (frequent[script] ?? 0) + 1;
			}
		}
	}

	for (const match of text.matchAll(WORD)) {
		penalty += wordPenalty(match[0]);
	}
	penalty += 4 * countSymbolsInWords([...text]);

	// Running text uses its alphabet's most frequent letters most of the time; a decoding
	// into the wrong alphabet (Hebrew read as Cyrillic, say) mostly doesn't.
	for (const script of Object.keys(letters)) {
		const count = letters[script];
		if (count >= 20 && (frequent[script] ?? 0) / count < FREQUENT_MIN_SHARE) {
			penalty += Math.round(count / 2);
		}
	}

	if (JAPANESE.has(encoding) && han >= 8 && kana === 0) {
		penalty += 2 * han; // Japanese text without any kana
	}
	if (encoding === 'euckr' && hangul === 0 && han > 0) {
		penalty += 2 * han; // Korean text without any Hangul
	}
	if (encoding === 'gb2312') {
		penalty += 2 * countLowTrailBytes(bytes); // common in Big5, absent from GB2312
	}
	return penalty;
}

function wordPenalty(word: string): number {
	const letters = [...word].filter(ch => /\p{L}/u.test(ch));
	if (letters.every(ch => ch < '\u0080')) {
		return 0;
	}
	let penalty = 0;
	const scripts = new Set(letters.map(scriptOf));
	if (scripts.size > 1) {
		penalty += 6; // two alphabets in one word
	}
	const [only] = scripts;
	if (scripts.size === 1 && (only === 'Latn' || only === 'Grek' || only === 'Cyrl') && letters.length >= 2) {
		const upper = letters.filter(isUpper).length;
		const lower = letters.filter(isLower).length;
		const caseOk = upper === 0 || lower === 0 || (upper === 1 && isUpper(letters[0]));
		if (!caseOk) {
			penalty += 3; // e.g. "пРИВЕТ": KOI8-R read as windows-1251
		} else if (lower === 0 && letters.length >= 3) {
			penalty += 1; // all-capital words are rare in running text
		}
		if (only === 'Latn' && letters.length >= 3) {
			const accented = letters.filter(ch => ch >= '\u0080').length;
			if (accented * 2 > letters.length) {
				penalty += 3; // e.g. "Ïðèâåò": Cyrillic read as windows-1252
			}
		}
	}
	return penalty;
}

/** Double-byte characters whose second byte is below 0xA1: valid Big5, never GB2312. */
function countLowTrailBytes(bytes: Uint8Array): number {
	let count = 0;
	for (let i = 0; i < bytes.length - 1; i++) {
		if (bytes[i] >= 0x81) {
			if (bytes[i + 1] < 0xa1) {
				count++;
			}
			i++;
		}
	}
	return count;
}
