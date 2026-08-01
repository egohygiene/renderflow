import { screen } from "@testing-library/react";
import { describe, expect, test } from "vitest";

import { renderRoute } from "./render-app";

describe("route rendering", () => {
	test("renders the home route at /renderflow/", async () => {
		renderRoute("/renderflow/");
		expect(
			await screen.findByRole("heading", { name: "Spec-driven document rendering engine" }),
		).toBeInTheDocument();
	});

	test("renders the examples route directly", async () => {
		renderRoute("/renderflow/examples");
		expect(
			await screen.findByRole("heading", { name: "Verified workflows from the repository" }),
		).toBeInTheDocument();
	});

	test("renders the architecture route directly", async () => {
		renderRoute("/renderflow/architecture");
		expect(
			await screen.findByRole("heading", {
				name: "Product-level architecture, not a second implementation",
			}),
		).toBeInTheDocument();
	});

	test("renders the not found route for unknown pages", async () => {
		renderRoute("/renderflow/missing");
		expect(
			await screen.findByRole("heading", { name: "That Renderflow page does not exist" }),
		).toBeInTheDocument();
	});
});
