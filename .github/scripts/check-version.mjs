import { readFileSync, appendFileSync } from 'node:fs';

const pkg = JSON.parse(readFileSync('package.json', 'utf8'));
const tauri = JSON.parse(readFileSync('src-tauri/tauri.conf.json', 'utf8'));
const cargo = readFileSync('src-tauri/Cargo.toml', 'utf8');
const packageSection = cargo.split('[package]')[1]?.split(/^\[/m)[0];
const cargoVersion = packageSection?.match(/^version\s*=\s*"([^"]+)"/m)?.[1];

if (!/^\d+\.\d+\.\d+$/.test(pkg.version) || pkg.version !== tauri.version || pkg.version !== cargoVersion) {
  throw new Error(`Application versions must match: package.json=${pkg.version}, tauri=${tauri.version}, Cargo=${cargoVersion}`);
}
const ref = process.env.GITHUB_REF ?? '';
if (ref.startsWith('refs/tags/') && ![`refs/tags/${pkg.version}`, `refs/tags/v${pkg.version}`].includes(ref)) {
  throw new Error(`Release tag must be ${pkg.version} or v${pkg.version}, got ${ref.slice('refs/tags/'.length)}`);
}
if (process.env.GITHUB_OUTPUT) appendFileSync(process.env.GITHUB_OUTPUT, `version=${pkg.version}\n`);
console.log(`Application version: ${pkg.version}; ref: ${ref || 'local'}`);
