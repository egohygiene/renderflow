import { describe, expect, test } from "vitest";

import {
	publicBasePath,
	resolvePublicPath,
	routerBasename,
	siteManifest,
} from "../app/site-config";

describe("site manifest loading", () => {
	test("keeps the app contract aligned", () => {
		expect(siteManifest.product).toBe("renderflow");
		expect(siteManifest.basePath).toBe("/renderflow/");
		expect(publicBasePath).toBe("/renderflow/");
		expect(routerBasename).toBe("/renderflow");
		expect(siteManifest.routes).toContain("/renderflow/examples");
		expect(resolvePublicPath("assets/example.svg")).toBe("/renderflow/assets/example.svg");
		expect(siteManifest.documentation.home).toBe("https://egohygiene.github.io/renderflow/");
	});
});
