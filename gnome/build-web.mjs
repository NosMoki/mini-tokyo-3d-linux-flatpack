// Stages the Mini Tokyo 3D web app for the GNOME desktop client.
//
//   node gnome/build-web.mjs [--reuse-dist]
//
// The result in gnome/_build/web is the same page that minitokyo3d.com serves,
// built from this repository with the repository's own scripts, plus the five
// plugins that the website loads. The web code itself is not changed.
//
// 1. `npm run build` bundles the core library into dist/ (skipped with
//    --reuse-dist when dist/ is already built).
// 2. The plugins are downloaded from the npm registry at the pinned versions
//    below, and checked against their integrity hashes.
// 3. public/, the bundle and its assets are copied, and index.html is generated
//    by scripts/gen-index.mjs. It runs in an empty staging directory so that no
//    .env file or analytics id from the developer's checkout ends up in the app.
//    The access tokens are written as placeholders, which the desktop app fills
//    in from GNOME Keyring when it serves the page.

import {execFileSync} from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

const gnomeDir = path.dirname(fileURLToPath(import.meta.url));
const rootDir = path.dirname(gnomeDir);
const buildDir = path.join(gnomeDir, '_build');
const cacheDir = path.join(buildDir, 'cache');
const stageDir = path.join(buildDir, 'stage');
const webDir = path.join(buildDir, 'web');

// Must match the placeholders in gnome/src/server.rs.
const PLACEHOLDERS = {
    MAPBOX_ACCESS_TOKEN: 'MT3D_DESKTOP_MAPBOX_ACCESS_TOKEN',
    MT3D_SECRET_ODPT: 'MT3D_DESKTOP_ODPT_ACCESS_TOKEN',
    MT3D_SECRET_CHALLENGE: 'MT3D_DESKTOP_CHALLENGE_ACCESS_TOKEN'
};

// The plugins that minitokyo3d.com loads, keyed by the variable in
// scripts/plugins.mjs that enables each one.
const PLUGINS = [{
    env: 'MT3D_PLUGIN_PRECIPITATION',
    name: 'mt3d-plugin-precipitation',
    version: '2.0.0',
    integrity: 'sha512-gDaujUHopNIpt4MWAExX7hsZRUJrgty6pd6iVnPpqoDQzp0q/GAC6f/ZiVNIpMt/ot4xMhBIBZLN5RZr+57TiA=='
}, {
    env: 'MT3D_PLUGIN_FIREWORKS',
    name: 'mt3d-plugin-fireworks',
    version: '2.1.0',
    integrity: 'sha512-eDjvok0PzZsQPQ8CZD8q+DUUchwuMecEBcAcwwFflopJJOjyOeCabVC74lmKEivPaDC5un/Aoyb4ui1lnCS3+g=='
}, {
    env: 'MT3D_PLUGIN_LIVECAM',
    name: 'mt3d-plugin-livecam',
    version: '2.0.0',
    integrity: 'sha512-K7W/NumD3bdXDy0+JPVck0wMU9t11/CrMjFwU57X7I1mr3q7wxot2ieHaKQuM/8yKg0Ega74eJJKc4TlU68acg=='
}, {
    env: 'MT3D_PLUGIN_PLATEAU',
    name: 'mt3d-plugin-plateau',
    version: '1.1.0',
    integrity: 'sha512-xiWeGBr4tS9z9XoKGY9fZ/YLFa9mKn3dhl3rqOc1KNbYiyyDysOTkTsqIBifmcLUSGXw4Mthk2TwtRb/Nn+Ivw=='
}, {
    env: 'MT3D_PLUGIN_GTFS',
    name: 'mt3d-plugin-gtfs',
    version: '1.0.0',
    integrity: 'sha512-5ReMlt8r4eroBpDlw90srhLi8ZwBC619S8a4JfxPKAgE5Hdkq85DlSOItrTHR9vGLhIos2OUSbTmi9Sr0U+qow=='
}];

const run = (command, args, options) => {
    console.log(`> ${command} ${args.join(' ')}`);
    execFileSync(command, args, {stdio: 'inherit', ...options});
};

const checkIntegrity = (data, integrity) => {
    const [algorithm, expected] = integrity.split('-');
    const actual = crypto.createHash(algorithm).update(data).digest('base64');
    return actual === expected;
};

async function fetchPlugin({name, version, integrity}) {
    const tarball = path.join(cacheDir, `${name}-${version}.tgz`);
    let data = fs.existsSync(tarball) ? fs.readFileSync(tarball) : undefined;

    if (!data || !checkIntegrity(data, integrity)) {
        const url = `https://registry.npmjs.org/${name}/-/${name}-${version}.tgz`;
        console.log(`Downloading ${url}`);
        const response = await fetch(url);
        if (!response.ok) {
            throw new Error(`${url}: HTTP ${response.status}`);
        }
        data = Buffer.from(await response.arrayBuffer());
        if (!checkIntegrity(data, integrity)) {
            throw new Error(`${name}@${version} does not match its integrity hash`);
        }
        fs.writeFileSync(tarball, data);
    }

    const file = `${name}.min.js`;
    const outDir = path.join(cacheDir, `${name}-${version}`);
    fs.rmSync(outDir, {recursive: true, force: true});
    fs.mkdirSync(outDir, {recursive: true});
    run('tar', ['-xzf', tarball, '-C', outDir, '--strip-components=2', `package/dist/${file}`]);
    return path.join(outDir, file);
}

const reuseDist = process.argv.includes('--reuse-dist');
const coreFiles = ['mini-tokyo-3d.min.js', 'mini-tokyo-3d.min.css'];

if (!reuseDist || !coreFiles.every(file => fs.existsSync(path.join(rootDir, 'dist', file)))) {
    run('npm', ['run', 'build'], {cwd: rootDir});
}

fs.mkdirSync(cacheDir, {recursive: true});
const pluginEnv = {};
for (const plugin of PLUGINS) {
    pluginEnv[plugin.env] = await fetchPlugin(plugin);
}

// Generate index.html with the repository's own generator, isolated from the
// checkout's .env files.
fs.rmSync(stageDir, {recursive: true, force: true});
fs.mkdirSync(path.join(stageDir, 'public'), {recursive: true});
fs.copyFileSync(path.join(rootDir, 'public', 'index.html'), path.join(stageDir, 'public', 'index.html'));
run(process.execPath, [path.join(rootDir, 'scripts', 'gen-index.mjs'), 'prod'], {
    cwd: stageDir,
    env: {
        ...process.env,
        ...PLACEHOLDERS,
        ...pluginEnv,
        MT3D_GA_ID: '',
        MT3D_DATA_URL: ''
    }
});

// Assemble the site the way scripts/build-pages.mjs does.
fs.rmSync(webDir, {recursive: true, force: true});
fs.mkdirSync(webDir, {recursive: true});
for (const entry of fs.readdirSync(path.join(rootDir, 'public'))) {
    if (entry !== 'index.html') {
        fs.cpSync(path.join(rootDir, 'public', entry), path.join(webDir, entry), {recursive: true});
    }
}
for (const file of coreFiles) {
    fs.copyFileSync(path.join(rootDir, 'dist', file), path.join(webDir, file));
}
fs.cpSync(path.join(rootDir, 'dist', 'assets'), path.join(webDir, 'assets'), {recursive: true});
fs.cpSync(path.join(stageDir, 'build'), webDir, {recursive: true});
fs.rmSync(stageDir, {recursive: true, force: true});

const html = fs.readFileSync(path.join(webDir, 'index.html'), 'utf8');
for (const placeholder of Object.values(PLACEHOLDERS)) {
    if (!html.includes(JSON.stringify(placeholder))) {
        throw new Error(`index.html is missing the ${placeholder} placeholder`);
    }
}
for (const {name} of PLUGINS) {
    if (!html.includes(`<script src="${name}.min.js"></script>`)) {
        throw new Error(`index.html does not load ${name}`);
    }
}

console.log(`Staged the web app in ${path.relative(rootDir, webDir)}/`);
