import { screen } from "@testing-library/react";
import axe from "axe-core";
import { afterEach, describe, expect, test } from "vitest";
import { cleanup } from "@testing-library/react";

import { renderRoute } from "./render-app";

afterEach(() => {
	cleanup();
});

const routeAssertions = [
	{
		path: "/renderflow/",
		heading: "Spec-driven document rendering engine",
	},
	{
		path: "/renderflow/examples",
		heading: "Verified workflows from the repository",
	},
	{
		path: "/renderflow/architecture",
		heading: "Product-level architecture, not a second implementation",
	},
] as const;

describe("accessibility checks", () => {
	test.each(routeAssertions)(
		"has no serious accessibility violations for $path",
		async ({ path, heading }) => {
			const { container } = renderRoute(path);
			expect(await screen.findByRole("heading", { name: heading })).toBeInTheDocument();

			const results = await axe.run(container);

			expect(results.violations).toEqual([]);
		},
	);
});
