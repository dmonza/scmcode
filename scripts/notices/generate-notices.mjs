#!/usr/bin/env node
// Generates THIRD_PARTY_NOTICES.txt: the copyright notices and license texts of the
// third-party software included in scmCode's installers.
//
//   npm run notices          write THIRD_PARTY_NOTICES.txt
//   npm run notices:check    fail if THIRD_PARTY_NOTICES.txt is out of date (CI)
//
// Both fail when a component is outside the license policy (scripts/notices/policy.json)
// or when a notice cannot be found. Inputs, all required:
//   - npm packages      package-lock.json + node_modules (run `npm ci` first)
//   - extensions        extensions/ (run `bash scripts/setup-extensions.sh` first)
//   - Rust crates       cargo-about at the version pinned in policy.json, about.toml,
//                       scripts/notices/about.hbs (license files are read from the crates
//                       in the local Cargo registry; nothing is fetched)
//   - repository files  scripts/notices/embedded.json
//
// The output depends only on these inputs (no dates, no host-specific data), so CI can
// regenerate it and compare.

import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import { dirname, join, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(HERE, '..', '..');
const OUTPUT = join(ROOT, 'THIRD_PARTY_NOTICES.txt');
const CHECK = process.argv.includes('--check');

const RULE = '='.repeat(79);
const SUBRULE = '-'.repeat(79);

const errors = [];
const error = (message) => errors.push(message);

// ── helpers ──────────────────────────────────────────────────────────────────

function readJson(path) {
  return JSON.parse(readFileSync(path, 'utf8').replace(/^﻿/, ''));
}

/** Same text on every OS: no BOM, LF line endings, no trailing blanks, one final newline. */
function normalizeText(text) {
  return (
    text
      .replace(/^﻿/, '')
      .replace(/\r\n?/g, '\n')
      .replace(/[ \t]+$/gm, '')
      .replace(/^\n+/, '')
      .replace(/\n*$/, '') + '\n'
  );
}

const readText = (path) => normalizeText(readFileSync(path, 'utf8'));
/** Code-point order: the same on every machine, unlike localeCompare. */
const compare = (a, b) => (a < b ? -1 : a > b ? 1 : 0);
const rel = (path) => relative(ROOT, path).split(sep).join('/');

function walk(dir, onFile, skip = () => false) {
  for (const name of readdirSync(dir).sort()) {
    const path = join(dir, name);
    if (skip(name)) continue;
    if (statSync(path).isDirectory()) walk(path, onFile, skip);
    else onFile(path);
  }
}

/**
 * Evaluates an SPDX license expression against a set of allowed ids: OR needs one allowed
 * side, AND needs both. "X WITH Y" is matched as a whole.
 */
function licenseAllowed(expression, allowed) {
  const tokens = String(expression).replace(/[()]/g, ' $& ').trim().split(/\s+/);
  let i = 0;
  const parseOr = () => {
    let value = parseAnd();
    while (tokens[i] === 'OR') {
      i++;
      const right = parseAnd();
      value = value || right;
    }
    return value;
  };
  const parseAnd = () => {
    let value = parseAtom();
    while (tokens[i] === 'AND') {
      i++;
      const right = parseAtom();
      value = value && right;
    }
    return value;
  };
  const parseAtom = () => {
    const token = tokens[i++];
    if (token === '(') {
      const value = parseOr();
      if (tokens[i++] !== ')') throw new Error(`unbalanced parentheses in "${expression}"`);
      return value;
    }
    if (!token || ['(', ')', 'AND', 'OR', 'WITH'].includes(token)) {
      throw new Error(`cannot parse license expression "${expression}"`);
    }
    let id = token;
    if (tokens[i] === 'WITH') {
      i++;
      id = `${token} WITH ${tokens[i++]}`;
    }
    return allowed.has(id);
  };
  const value = parseOr();
  if (i !== tokens.length) throw new Error(`cannot parse license expression "${expression}"`);
  return value;
}

function checkLicense(what, expression, allowed) {
  try {
    if (!licenseAllowed(expression, allowed)) {
      error(`${what}: license "${expression}" is not allowed by scripts/notices/policy.json.`);
    }
  } catch (e) {
    error(`${what}: ${e.message}`);
  }
}

function block(title, lines, text) {
  return [SUBRULE, title, ...lines.filter(Boolean), '', text.trimEnd(), ''].join('\n');
}

// ── policy ───────────────────────────────────────────────────────────────────

const policy = readJson(join(HERE, 'policy.json'));
const ALLOWED = new Set(policy.allowedLicenses);
const ALLOWED_ASSETS = new Set([
  ...ALLOWED,
  ...Object.keys(policy.allowedAssetLicenses).filter((k) => !k.startsWith('$')),
]);

function checkPolicyMatchesDenyToml() {
  const deny = readFileSync(join(ROOT, 'deny.toml'), 'utf8');
  const section = deny.match(/^\[licenses\]\s*$([\s\S]*?)(?=^\[|(?![\s\S]))/m);
  const list = section && section[1].match(/^allow\s*=\s*\[([\s\S]*?)\]/m);
  if (!list) {
    error('deny.toml: could not find [licenses] allow = [...].');
    return;
  }
  const denyAllowed = [...list[1].matchAll(/"([^"]+)"/g)].map((m) => m[1]).sort();
  const policyAllowed = [...ALLOWED].sort();
  if (JSON.stringify(denyAllowed) !== JSON.stringify(policyAllowed)) {
    error(
      'deny.toml [licenses] allow and scripts/notices/policy.json allowedLicenses differ:\n' +
        `  deny.toml:   ${denyAllowed.join(', ')}\n  policy.json: ${policyAllowed.join(', ')}`,
    );
  }
}

// ── 1. files kept in the repository ─────────────────────────────────────────

function repositorySection() {
  const { components } = readJson(join(HERE, 'embedded.json'));
  const listed = new Set();
  const blocks = [];

  for (const c of components) {
    const what = `embedded.json "${c.name}"`;
    checkLicense(what, c.license, ALLOWED_ASSETS);
    for (const f of c.files) {
      if (!existsSync(join(ROOT, f))) error(`${what}: ${f} does not exist.`);
    }
    for (const m of c.cgmanifests ?? []) listed.add(m);
    const texts = [];
    for (const f of c.licenseFiles) {
      const path = join(ROOT, f);
      if (!existsSync(path)) {
        error(`${what}: license file ${f} does not exist${f.startsWith('node_modules/') ? ' (run npm ci)' : ''}.`);
        continue;
      }
      texts.push(readText(path));
    }
    blocks.push(
      block(
        `${c.name} ${c.version}`.trim(),
        [c.url, `License: ${c.license}`, c.note, `Included as: ${c.files.join(', ')}`],
        texts.join('\n'),
      ),
    );
  }

  // Every third-party component that Code - OSS registered under src/ must be listed.
  walk(
    join(ROOT, 'src'),
    (path) => {
      if (path.endsWith(`${sep}cgmanifest.json`) && !listed.has(rel(path))) {
        error(`${rel(path)} declares third-party code that scripts/notices/embedded.json does not list.`);
      }
    },
    (name) => name === 'node_modules',
  );

  return { blocks, count: components.length };
}

// ── 2. npm packages ─────────────────────────────────────────────────────────

const LICENSE_FILE =
  /^(licen[cs]e|copying|notices?|third[-_]?party[-_]?notices?)([-_.][^/]*)?$/i;

function npmLicenseFiles(dir) {
  return readdirSync(dir)
    .filter((name) => LICENSE_FILE.test(name) && !/\.(spdx|json|js|ts)$/i.test(name))
    .filter((name) => statSync(join(dir, name)).isFile())
    .sort();
}

/**
 * Section 2 covers "dependencies" only. Vite bundles whatever the source imports, so a
 * devDependency imported for its value would ship without a notice: refuse that.
 * `import type` is erased at build time and is allowed.
 */
function checkNoBundledDevDependencies() {
  const pkg = readJson(join(ROOT, 'package.json'));
  const dev = new Set(Object.keys(pkg.devDependencies ?? {}));
  const IMPORT = /(import\s+type\b[^;]*?\bfrom\s*|\bfrom\s*|\bimport\s*\(\s*|\brequire\(\s*)['"]((?:@[^'"/]+\/)?[^'"/]+)/g;
  walk(
    join(ROOT, 'src'),
    (path) => {
      if (!/\.(m?[jt]sx?)$/.test(path)) return;
      for (const m of readFileSync(path, 'utf8').matchAll(IMPORT)) {
        if (dev.has(m[2]) && !/^import\s+type\b/.test(m[1])) {
          error(`${rel(path)} imports devDependency "${m[2]}", which would be bundled. Move it to "dependencies".`);
        }
      }
    },
    (name) => name === 'node_modules',
  );
}

function npmSection() {
  checkNoBundledDevDependencies();
  const lock = readJson(join(ROOT, 'package-lock.json'));
  const overrides = policy.npm.licenseTexts;
  const blocks = [];

  const packages = Object.entries(lock.packages)
    .filter(([key, meta]) => key && !meta.dev)
    .map(([key, meta]) => ({ key, meta, name: meta.name ?? key.replace(/^.*node_modules\//, '') }))
    .sort((a, b) => compare(a.name, b.name) || compare(a.key, b.key));

  for (const { key, meta, name } of packages) {
    const dir = join(ROOT, key);
    const what = `npm ${name}@${meta.version}`;
    if (!existsSync(dir)) {
      if (meta.optional) continue; // a platform-specific build for another OS
      error(`${what}: not installed (run npm ci).`);
      continue;
    }
    checkLicense(what, meta.license ?? 'UNKNOWN', ALLOWED_ASSETS);

    const pkg = readJson(join(dir, 'package.json'));
    const repo = typeof pkg.repository === 'string' ? pkg.repository : pkg.repository?.url;
    const url = (repo ?? pkg.homepage ?? '').replace(/^git\+/, '').replace(/\.git$/, '');

    let parts = npmLicenseFiles(dir).map((f) => ({ name: f, text: readText(join(dir, f)) }));
    const override = overrides[name];
    if (override) {
      if (override.file) {
        parts.push({ name: rel(join(HERE, override.file)), text: readText(join(HERE, override.file)) });
      } else if (override.package) {
        const other = join(ROOT, 'node_modules', override.package);
        parts = npmLicenseFiles(other).map((f) => ({
          name: `${override.package}/${f}`,
          text: readText(join(other, f)),
        }));
      }
    }
    if (parts.length === 0) {
      error(`${what}: ships no license text. Add an entry to npm.licenseTexts in scripts/notices/policy.json.`);
      continue;
    }
    const text =
      parts.length === 1
        ? parts[0].text
        : parts.map((p) => `[${p.name}]\n\n${p.text}`).join('\n');
    blocks.push(block(`${name} ${meta.version}`, [url, `License: ${meta.license}`], text));
  }
  return { blocks, count: blocks.length };
}

// ── 3. Code - OSS and the built-in extensions ───────────────────────────────

function extensionsSection() {
  const extDir = join(ROOT, 'extensions');
  const setup = readFileSync(join(ROOT, 'scripts', 'setup-extensions.sh'), 'utf8');
  const commit = setup.match(/^CODE_OSS_COMMIT="([0-9a-f]{40})"/m)?.[1];
  const version = setup.match(/^CODE_OSS_VERSION="([^"]+)"/m)?.[1];
  const allowlist = readFileSync(join(ROOT, 'scripts', 'builtin-extensions.txt'), 'utf8')
    .split(/\r?\n/)
    .map((l) => l.replace(/#.*/, '').trim())
    .filter(Boolean);

  const stampPath = join(extDir, '.scmcode-source');
  const expectedStamp = `code-oss-version=${version}\ncode-oss-commit=${commit}\nextensions=${allowlist.join(',')}\n`;
  if (!existsSync(stampPath) || normalizeText(readFileSync(stampPath, 'utf8')) !== expectedStamp) {
    error('extensions/ is missing or out of date. Run: bash scripts/setup-extensions.sh');
    return { blocks: [], count: 0 };
  }

  const licenseNames = policy.extensions.licenseNames;
  const allowedRefs = new Set(Object.keys(policy.extensions.allowedLicenseRefs));
  const allowed = new Set([...ALLOWED_ASSETS, ...allowedRefs]);
  const unlabeled = policy.extensions.unlabeledComponents;
  const supplements = policy.extensions.supplements;

  const codeOssLicense = readText(join(extDir, 'LICENSE-Code-OSS.txt'));
  const codeOssNotices = readText(join(extDir, 'ThirdPartyNotices-Code-OSS.txt'));
  const noticeSections = codeOssNotices.split(/\n-{20,}\n/).map((s) => s.trim());
  const sectionHeads = noticeSections.map((s) => s.split('\n')[0].toLowerCase());
  const normUrl = (u) => (u ?? '').trim().toLowerCase().replace(/\.git$/, '').replace(/\/+$/, '');
  const sectionUrls = noticeSections.map((s) => normUrl(s.split('\n')[1]));

  // component key -> { title, lines, text, extensions[] }
  const components = new Map();
  const localFiles = [];

  for (const ext of allowlist) {
    const dir = join(extDir, ext);
    walk(
      dir,
      (path) => {
        const name = path.slice(path.lastIndexOf(sep) + 1);
        if (name === 'cgmanifest.json') {
          for (const r of readJson(path).registrations ?? []) {
            const c = r.component ?? {};
            const info = c[c.type] ?? {};
            const url = info.repositoryUrl ?? info.downloadUrl ?? '';
            const compName = info.name ?? url.replace(/\/+$/, '').split('/').pop();
            const compVersion = r.version ?? info.tag ?? info.commitHash ?? '';
            const what = `extension ${ext}: component "${compName}"`;

            const rawLicense = typeof r.license === 'object' && r.license ? r.license.type : r.license;
            const spdx = rawLicense ? (licenseNames[rawLicense] ?? rawLicense) : unlabeled[compName];
            if (!spdx) {
              error(`${what} has no license and is not listed in extensions.unlabeledComponents.`);
              continue;
            }
            checkLicense(what, spdx, allowed);

            let text;
            if (Array.isArray(r.licenseDetail) && r.licenseDetail.length) {
              text = normalizeText(r.licenseDetail.join('\n'));
            } else {
              // Code - OSS's ThirdPartyNotices.txt (reproduced in full below) has the text.
              const lower = compName.toLowerCase();
              const found =
                sectionHeads.some((h) => h === lower || h.startsWith(`${lower} `)) ||
                (url && sectionUrls.includes(normUrl(url)));
              if (!found) error(`${what}: no license text in cgmanifest.json or in Code - OSS ThirdPartyNotices.txt.`);
            }
            if (supplements[compName]) {
              const extra = readText(join(HERE, supplements[compName]));
              text = (text ? `${text}\n` : '') + extra;
            }
            if (!text) continue;
            const key = `${compName}\n${normUrl(url)}\n${text}`;
            if (!components.has(key)) {
              components.set(key, { title: `${compName} ${compVersion}`.trim(), url, spdx, text, extensions: [] });
            }
            const entry = components.get(key);
            if (!entry.extensions.includes(ext)) entry.extensions.push(ext);
          }
        } else if (/^((.*[-_.])?(licen[cs]e|notices?)|thirdpartynotices)\.(txt|md)$/i.test(name)) {
          localFiles.push({ path, text: readText(path) });
        }
      },
      (name) => name === 'node_modules',
    );
  }

  const blocks = [];
  blocks.push(
    block(
      `Code - OSS ${version}`,
      [
        'https://github.com/microsoft/vscode',
        `Commit: ${commit}`,
        'License: MIT',
        `Covers scmCode's workbench (derived from Code - OSS) and the ${allowlist.length} built-in extensions:`,
        wrapList(allowlist),
      ],
      codeOssLicense,
    ),
  );
  for (const c of [...components.values()].sort((a, b) => compare(a.title, b.title))) {
    blocks.push(
      block(c.title, [c.url, `License: ${c.spdx}`, `Used by the built-in extensions: ${c.extensions.join(', ')}`], c.text),
    );
  }
  for (const f of localFiles) {
    blocks.push(block(rel(f.path), ['Notice file shipped inside the built-in extension.'], f.text));
  }
  blocks.push(
    block(
      `Code - OSS ${version} ThirdPartyNotices.txt`,
      ['Reproduced in full. It covers third-party material in Code - OSS, including the built-in extensions above.'],
      codeOssNotices,
    ),
  );
  return { blocks, count: allowlist.length };
}

function wrapList(items) {
  const lines = [];
  let line = ' ';
  for (const item of items) {
    if (line.length + item.length + 2 > 79) {
      lines.push(line.trimEnd());
      line = ' ';
    }
    line += ` ${item},`;
  }
  lines.push(line.replace(/,$/, '').trimEnd());
  return lines.join('\n');
}

// ── 4. Rust crates ──────────────────────────────────────────────────────────

function cargo(args) {
  return execFileSync('cargo', args, {
    cwd: ROOT,
    encoding: 'utf8',
    maxBuffer: 256 * 1024 * 1024,
    stdio: ['ignore', 'pipe', 'pipe'],
  });
}

function rustSection() {
  const pinned = policy.tools['cargo-about'];
  let installed;
  try {
    installed = cargo(['about', '--version']).trim().split(/\s+/).pop();
  } catch {
    error(`cargo-about is not installed. Run: cargo install cargo-about --locked --features cli --version ${pinned}`);
    return { blocks: [], count: 0 };
  }
  if (installed !== pinned) {
    error(`cargo-about ${installed} is installed; the notices are generated with ${pinned}. Run: cargo install cargo-about --locked --features cli --version ${pinned}`);
    return { blocks: [], count: 0 };
  }

  let listing;
  try {
    listing = cargo([
      'about', 'generate', '--fail', '--locked',
      '--manifest-path', 'src-tauri/Cargo.toml',
      '-c', 'about.toml',
      'scripts/notices/about.hbs',
    ]);
  } catch (e) {
    error(`cargo about generate failed:\n${e.stderr || e.message}`);
    return { blocks: [], count: 0 };
  }
  listing = normalizeText(listing);

  // cargo-about also lists scmCode's own workspace crates (and attributes to them any
  // license file found in their folders, such as src-tauri/shell-integration/). They are
  // not third-party: drop them, and any license group left without crates.
  const metadata = JSON.parse(
    cargo(['metadata', '--locked', '--format-version', '1', '--manifest-path', 'src-tauri/Cargo.toml']),
  );
  const local = new Set(metadata.packages.filter((p) => p.source === null).map((p) => `${p.name} ${p.version}`));
  const groups = listing.split(`${SUBRULE}\n`);
  listing = [
    groups[0],
    ...groups.slice(1).flatMap((g) => {
      const lines = g.split('\n');
      const kept = lines.filter((l, i) => !(i > 2 && l.startsWith('  - ') && local.has(l.slice(4).split(' ').slice(0, 2).join(' '))));
      return kept.some((l) => l.startsWith('  - ')) ? [kept.join('\n')] : [];
    }),
  ].join(`${SUBRULE}\n`);

  // Crates that ship extra notice files (NOTICE, THIRD_PARTY...) besides their license.
  const crates = new Set();
  let inUsedBy = false;
  for (const line of listing.split('\n')) {
    if (line === 'Used by:') inUsedBy = true;
    else if (!line.startsWith('  - ')) inUsedBy = false;
    else if (inUsedBy) crates.add(line.slice(4).split(' ').slice(0, 2).join(' '));
  }
  const extra = [];
  const NOTICE = /^(notices?|third[-_]?party([-_]?notices)?)(\.(txt|md))?$/i;
  for (const p of [...metadata.packages].sort((a, b) => compare(a.name, b.name) || compare(a.version, b.version))) {
    if (!crates.has(`${p.name} ${p.version}`)) continue;
    const dir = dirname(p.manifest_path);
    for (const name of readdirSync(dir).sort()) {
      if (!NOTICE.test(name)) continue;
      const path = join(dir, name);
      if (statSync(path).isFile()) {
        extra.push(block(`${p.name} ${p.version}: ${name}`, [], readText(path)));
      } else {
        walk(path, (f) => {
          if (/^(licen[cs]e|copying|notice)/i.test(f.slice(f.lastIndexOf(sep) + 1))) {
            extra.push(block(`${p.name} ${p.version}: ${relative(dir, f).split(sep).join('/')}`, [], readText(f)));
          }
        });
      }
    }
  }

  const blocks = [listing.trimEnd() + '\n'];
  if (extra.length) {
    blocks.push(`${SUBRULE}\nAdditional notice files shipped with the crates above\n`, ...extra);
  }
  return { blocks, count: crates.size };
}

// ── assemble ────────────────────────────────────────────────────────────────

function exitOnErrors() {
  if (!errors.length) return;
  console.error(`generate-notices: ${errors.length} problem(s):\n`);
  for (const e of errors) console.error(`  - ${e}`);
  process.exit(1);
}

checkPolicyMatchesDenyToml();
const repo = repositorySection();
const npm = npmSection();
const ext = extensionsSection();
exitOnErrors(); // before the slow cargo-about run
const rust = rustSection();
exitOnErrors();

const header = `scmCode THIRD-PARTY NOTICES
${RULE}

scmCode is distributed under the MIT License (see LICENSE). It is a derivative of SideX,
Copyright (c) 2025 - present Siden Technologies Inc., and of Code - OSS, Copyright (c)
2015 - present Microsoft Corporation, both under the MIT License (see LICENSE and
NOTICE.md).

scmCode includes the third-party software listed below. Each entry reproduces the
copyright notices and the license under which scmCode received it. Where a component is
offered under a choice of licenses, scmCode uses it under the license reproduced here.
These licenses apply to the components they name, not to scmCode as a whole. The source
of every component is available at the address given with it.

  1. Third-party files kept in the scmCode repository ......... ${repo.count} components
  2. npm packages bundled into the user interface ............. ${npm.count} packages
  3. Code - OSS and its built-in extensions ................... ${ext.count} extensions
  4. Rust crates compiled into the application ................ ${rust.count} crates

This file is generated by scripts/notices/generate-notices.mjs (npm run notices).
Do not edit it by hand.
`;

const section = (n, title, blocks) => [RULE, `${n}. ${title}`, RULE, '', ...blocks].join('\n');

const output = normalizeText(
  [
    header,
    section(1, 'Third-party files kept in the scmCode repository', repo.blocks),
    section(2, 'npm packages bundled into the user interface', npm.blocks),
    section(3, 'Code - OSS and its built-in extensions', ext.blocks),
    section(4, 'Rust crates compiled into the application', rust.blocks),
    `${RULE}\nEnd of scmCode third-party notices.\n`,
  ].join('\n'),
);

if (CHECK) {
  const current = existsSync(OUTPUT) ? normalizeText(readFileSync(OUTPUT, 'utf8')) : '';
  if (current !== output) {
    const a = current.split('\n');
    const b = output.split('\n');
    let line = 0;
    while (line < Math.max(a.length, b.length) && a[line] === b[line]) line++;
    console.error(
      `THIRD_PARTY_NOTICES.txt is out of date (first difference at line ${line + 1}).\n` +
        `  committed: ${JSON.stringify(a[line] ?? '<end of file>')}\n` +
        `  expected:  ${JSON.stringify(b[line] ?? '<end of file>')}\n` +
        'Run `npm run notices` and commit the result.',
    );
    process.exit(1);
  }
  console.log('THIRD_PARTY_NOTICES.txt is up to date.');
} else {
  writeFileSync(OUTPUT, output);
  console.log(
    `Wrote THIRD_PARTY_NOTICES.txt: ${repo.count} repository components, ${npm.count} npm packages, ` +
      `${ext.count} built-in extensions, ${rust.count} Rust crates.`,
  );
}
