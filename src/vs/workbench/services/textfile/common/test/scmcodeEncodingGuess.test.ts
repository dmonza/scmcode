/*---------------------------------------------------------------------------------------------
 *  scmCode: tests for scmcodeEncodingGuess.ts (docs/issues/closed/ISSUE-0017).
 *
 *  Each sample is encoded with iconv-lite (the decoder the app uses) in every encoding that
 *  can represent it, and the guess must decode the bytes back to the original text: a guess
 *  of an equivalent encoding (windows-1255 for ISO-8859-8 Hebrew, say) counts as right.
 *
 *  Run with:
 *    node --experimental-strip-types --test src/vs/workbench/services/textfile/common/test/scmcodeEncodingGuess.test.ts
 *--------------------------------------------------------------------------------------------*/

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { isUtf8, scmcodeGuessEncoding, type ScmcodeDecode } from '../scmcodeEncodingGuess.ts';

const require = createRequire(import.meta.url);
const iconv = require('@vscode/iconv-lite-umd') as {
	encode(text: string, encoding: string): Uint8Array;
	decode(bytes: Uint8Array, encoding: string): string;
};
const decode: ScmcodeDecode = (bytes, encoding) => iconv.decode(bytes, encoding);

const SAMPLES: { lang: string; text: string; encodings: string[] }[] = [
	{
		lang: 'Spanish',
		text: 'El niño comió una manzana junto a la ventana. ¿Dónde está la estación? Mañana iremos al pueblo con nuestros amigos.',
		encodings: ['windows1252']
	},
	{
		lang: 'French',
		text: "Le garçon a mangé une pomme près de la fenêtre. Où est la gare ? C'était très agréable, n'est-ce pas ?",
		encodings: ['windows1252']
	},
	{
		lang: 'German',
		text: 'Die Größe der Straße überrascht uns. Wir müssen morgen früh aufstehen, um den Zug nach München zu erreichen.',
		encodings: ['windows1252']
	},
	{
		lang: 'Polish',
		text: 'Zażółć gęślą jaźń. W Polsce mówi się po polsku, a łódź płynie po rzece obok starego młyna.',
		encodings: ['windows1250', 'iso88592']
	},
	{
		lang: 'Czech',
		text: 'Příliš žluťoučký kůň úpěl ďábelské ódy. Dnes je krásný den a slunce svítí nad městem.',
		encodings: ['windows1250', 'iso88592']
	},
	{
		lang: 'Russian',
		text: 'Привет, мир! Сегодня хорошая погода, и мы пойдём гулять в парк. Москва - столица России, там много музеев.',
		encodings: ['windows1251', 'koi8r', 'cp866', 'iso88595']
	},
	{
		lang: 'Ukrainian',
		text: 'Добрий день! Як справи? Київ є столицею України, і там багато гарних вулиць та старих церков.',
		encodings: ['windows1251', 'cp1125']
	},
	{
		lang: 'Greek',
		text: 'Καλημέρα σας! Η Αθήνα είναι η πρωτεύουσα της Ελλάδας και έχει πολλά μουσεία και όμορφα πάρκα.',
		encodings: ['windows1253', 'iso88597']
	},
	{
		lang: 'Hebrew',
		text: 'שלום עולם! ירושלים היא עיר עתיקה ויפה מאוד, ואנחנו אוהבים לטייל בה עם החברים שלנו.',
		encodings: ['windows1255', 'iso88598']
	},
	{
		lang: 'Japanese',
		text: 'こんにちは、世界。今日はとても良い天気ですね。東京駅で友達と会う予定です。',
		encodings: ['shiftjis', 'eucjp']
	},
	{
		lang: 'Korean',
		text: '안녕하세요, 세계! 오늘은 날씨가 정말 좋네요. 서울에서 친구를 만날 예정입니다.',
		encodings: ['euckr']
	},
	{
		lang: 'Simplified Chinese',
		text: '你好，世界！今天天气很好。我们明天去北京的公园散步，然后一起吃晚饭。',
		encodings: ['gb2312']
	},
	{
		lang: 'Traditional Chinese',
		text: '你好，世界！今天天氣很好。我們明天去台北的公園散步，然後一起吃晚飯。',
		encodings: ['cp950']
	}
];

for (const sample of SAMPLES) {
	for (const encoding of sample.encodings) {
		test(`${sample.lang} in ${encoding} is guessed so that it decodes back`, () => {
			const bytes = iconv.encode(sample.text, encoding);
			assert.equal(iconv.decode(bytes, encoding), sample.text, 'sample must be representable');
			const guessed = scmcodeGuessEncoding(bytes, decode);
			assert.ok(guessed, 'no guess');
			assert.equal(iconv.decode(bytes, guessed), sample.text, `guessed ${guessed}`);
		});
	}
	test(`${sample.lang} in UTF-8 is guessed as utf8`, () => {
		assert.equal(scmcodeGuessEncoding(new TextEncoder().encode(sample.text), decode), 'utf8');
	});
}

test('a single accented word in windows-1252 decodes back', () => {
	const bytes = iconv.encode('café', 'windows1252');
	assert.equal(iconv.decode(bytes, scmcodeGuessEncoding(bytes, decode)!), 'café');
});

test('pure ASCII gives no guess', () => {
	assert.equal(scmcodeGuessEncoding(new TextEncoder().encode('just ascii\nlines\n'), decode), null);
});

test('UTF-8 cut in the middle of a character at the end of the sample is still UTF-8', () => {
	const bytes = new TextEncoder().encode('año 日本');
	assert.ok(isUtf8(bytes.slice(0, bytes.length - 1)));
	assert.ok(!isUtf8(new Uint8Array([0x63, 0x61, 0x66, 0xe9]))); // "café" in windows-1252: no complete sequence before
	assert.ok(!isUtf8(new Uint8Array([0x61, 0xe9, 0x62]))); // "aéb" in windows-1252
	assert.ok(!isUtf8(new Uint8Array([0xc0, 0xaf]))); // overlong "/"
	assert.ok(!isUtf8(new Uint8Array([0xed, 0xa0, 0x80]))); // surrogate
});

test('candidates restrict the guess, and unknown candidates are ignored', () => {
	const bytes = iconv.encode('Привет, мир! Сегодня хорошая погода.', 'koi8r');
	assert.equal(scmcodeGuessEncoding(bytes, decode, ['koi8r', 'windows1252']), 'koi8r');
	assert.equal(scmcodeGuessEncoding(bytes, decode, ['nonsense']), 'koi8r');
	// Only UTF-8 allowed and the bytes aren't UTF-8: nothing to choose.
	assert.equal(scmcodeGuessEncoding(bytes, decode, ['utf8']), null);
});

test('a 64 KB sample (what VS Code passes) is guessed in well under 100 ms', () => {
	const text = SAMPLES.map(s => s.text).join('\n');
	const bytes = iconv.encode(text.repeat(Math.ceil(65536 / text.length)), 'gb18030').slice(0, 65536);
	const start = performance.now();
	scmcodeGuessEncoding(bytes, decode);
	assert.ok(performance.now() - start < 100, `${performance.now() - start} ms`);
});
