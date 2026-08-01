import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const appRoot = resolve(fileURLToPath(new URL(".", import.meta.url)), "..");
const distDirectory = resolve(appRoot, "dist");
const indexHtmlPath = resolve(distDirectory, "index.html");
const fallbackPath = resolve(distDirectory, "404.html");
const manifestPath = resolve(appRoot, "site-manifest.json");

function assert(condition, message) {
	if (!condition) {
		throw new Error(message);
	}
}

assert(existsSync(distDirectory), "Expected apps/web/dist to exist after build.");
assert(existsSync(indexHtmlPath), "Expected apps/web/dist/index.html to exist after build.");
assert(existsSync(fallbackPath), "Expected apps/web/dist/404.html to exist for SPA fallback.");

const indexHtml = readFileSync(indexHtmlPath, "utf8");
const siteManifest = JSON.parse(readFileSync(manifestPath, "utf8"));

assert(
	siteManifest.basePath === "/renderflow/",
	"Expected site manifest basePath to remain /renderflow/.",
);
assert(
	siteManifest.outputDirectory === "apps/web/dist",
	"Expected outputDirectory to match build output.",
);
assert(
	indexHtml.includes("/renderflow/assets/"),
	"Expected built asset URLs to stay under /renderflow/assets/.",
);
assert(
	!indexHtml.includes('="/assets/'),
	"Detected an escaped absolute asset path outside the /renderflow/ base path.",
);

for (const match of indexHtml.matchAll(/(?:src|href)="(\/renderflow\/assets\/[^"]+)"/g)) {
	const relativeAssetPath = match[1].replace("/renderflow/", "");
	const assetPath = resolve(distDirectory, relativeAssetPath);
	assert(existsSync(assetPath), `Expected referenced asset to exist: ${relativeAssetPath}`);
}

console.log("Verified build output, asset paths, site manifest, and SPA fallback files.");
