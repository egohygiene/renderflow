import { render, screen } from "@testing-library/react";
import { describe, expect, test } from "vitest";
import { MemoryRouter } from "react-router-dom";

import { CodeExample } from "../components/CodeExample";
import { Footer } from "../components/Footer";
import { Header } from "../components/Header";
import { ProductStatus } from "../components/ProductStatus";
import { SkipLink } from "../components/SkipLink";
import { capabilities } from "../content/capabilities";

describe("shared components", () => {
	test("renders the header navigation", () => {
		render(
			<MemoryRouter>
				<Header />
			</MemoryRouter>,
		);

		expect(screen.getByRole("navigation", { name: "Primary navigation" })).toBeInTheDocument();
		expect(screen.getByRole("link", { name: "Overview" })).toBeInTheDocument();
		expect(screen.getByRole("link", { name: "Examples" })).toBeInTheDocument();
		expect(screen.getByRole("link", { name: "Architecture" })).toBeInTheDocument();
	});

	test("renders the footer ecosystem links", () => {
		render(<Footer />);

		expect(screen.getByRole("link", { name: "Technical documentation" })).toHaveAttribute(
			"href",
			"https://egohygiene.github.io/renderflow/",
		);
		expect(screen.getByRole("link", { name: "Source repository" })).toHaveAttribute(
			"href",
			"https://github.com/egohygiene/renderflow",
		);
	});

	test("renders a visible skip link", () => {
		render(<SkipLink />);
		expect(screen.getByRole("link", { name: "Skip to main content" })).toHaveAttribute(
			"href",
			"#main-content",
		);
	});

	test("labels statuses accessibly", () => {
		render(<ProductStatus status="planned" />);
		expect(screen.getByText("Planned")).toBeInTheDocument();
		expect(screen.getByText("Feature status:")).toBeInTheDocument();
	});

	test("renders a real configuration example", () => {
		render(
			<CodeExample
				code={"input: report.md\noutput_dir: dist\noutputs:\n  - type: html"}
				title="renderflow.yaml"
			/>,
		);

		expect(screen.getByText("renderflow.yaml")).toBeInTheDocument();
		expect(screen.getByText(/input: report.md/)).toBeInTheDocument();
	});

	test("keeps capability data typed and non-duplicated", () => {
		expect(capabilities).toEqual(
			expect.arrayContaining([
				expect.objectContaining({ identifier: "yaml-spec", status: "available" }),
				expect.objectContaining({ identifier: "container-distribution", status: "planned" }),
			]),
		);
	});
});
