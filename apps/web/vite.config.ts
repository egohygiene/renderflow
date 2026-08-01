import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { resolve } from "node:path";
import { defineConfig, loadEnv } from "vite";
import react from "@vitejs/plugin-react";

const manifestPath = resolve(fileURLToPath(new URL(".", import.meta.url)), "site-manifest.json");
const siteManifest = JSON.parse(readFileSync(manifestPath, "utf8")) as {
	basePath: string;
};

function normalizePublicBasePath(basePath: string): string {
	const trimmed = basePath.trim().replace(/^\/+|\/+$/g, "");

	return trimmed === "" ? "/" : `/${trimmed}/`;
}

export default defineConfig(({ mode }) => {
	const env = loadEnv(mode, process.cwd(), "");
	const publicBasePath = normalizePublicBasePath(
		env.VITE_PUBLIC_BASE_PATH || siteManifest.basePath,
	);

	return {
		base: publicBasePath,
		build: {
			sourcemap: false,
		},
		plugins: [react()],
		test: {
			environment: "jsdom",
			setupFiles: ["./src/test/setup.ts"],
			css: true,
		},
	};
});
