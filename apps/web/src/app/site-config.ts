import siteManifestJson from "../../site-manifest.json";

type SiteManifest = {
	readonly schema: string;
	readonly product: string;
	readonly displayName: string;
	readonly basePath: string;
	readonly repository: string;
	readonly status: string;
	readonly buildCommand: string;
	readonly outputDirectory: string;
	readonly localDevelopmentCommand: string;
	readonly routes: readonly string[];
	readonly documentation: {
		readonly home: string;
		readonly installation: string;
		readonly quickstart: string;
		readonly architecture: string;
	};
	readonly deployment: {
		readonly requiredNodeVersion: string;
		readonly previewCommand: string;
		readonly smokeTestPath: string;
		readonly gatewayOwner: string;
		readonly gatewayPath: string;
		readonly spaFallback: string;
		readonly environmentVariables: readonly string[];
	};
};

export function normalizePublicBasePath(basePath: string): string {
	const trimmed = basePath.trim().replace(/^\/+|\/+$/g, "");

	return trimmed === "" ? "/" : `/${trimmed}/`;
}

export function toRouterBasename(basePath: string): string {
	const normalized = normalizePublicBasePath(basePath);
	return normalized === "/" ? normalized : normalized.replace(/\/$/, "");
}

export const siteManifest = siteManifestJson as SiteManifest;
const runtimeBasePath =
	import.meta.env.BASE_URL && import.meta.env.BASE_URL !== "/"
		? import.meta.env.BASE_URL
		: siteManifest.basePath;
export const publicBasePath = normalizePublicBasePath(runtimeBasePath);
export const routerBasename = toRouterBasename(publicBasePath);

export function resolvePublicPath(pathname: string): string {
	const trimmed = pathname.replace(/^\//, "");
	return trimmed.length === 0 ? publicBasePath : `${publicBasePath}${trimmed}`;
}
