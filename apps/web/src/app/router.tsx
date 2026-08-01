import { lazy, Suspense, type ReactNode } from "react";
import {
	RouterProvider,
	createBrowserRouter,
	createMemoryRouter,
	type RouteObject,
} from "react-router-dom";

import { App } from "./App";
import { routerBasename } from "./site-config";

const HomePage = lazy(() => import("../pages/HomePage"));
const ExamplesPage = lazy(() => import("../pages/ExamplesPage"));
const ArchitecturePage = lazy(() => import("../pages/ArchitecturePage"));
const NotFoundPage = lazy(() => import("../pages/NotFoundPage"));

function withSuspense(element: ReactNode) {
	return <Suspense fallback={<div className="page-loading">Loading…</div>}>{element}</Suspense>;
}

export const appRoutes: RouteObject[] = [
	{
		path: "/",
		element: <App />,
		children: [
			{ index: true, element: withSuspense(<HomePage />) },
			{ path: "examples", element: withSuspense(<ExamplesPage />) },
			{ path: "architecture", element: withSuspense(<ArchitecturePage />) },
			{ path: "*", element: withSuspense(<NotFoundPage />) },
		],
	},
];

export function createAppRouter() {
	return createBrowserRouter(appRoutes, { basename: routerBasename });
}

export function createAppMemoryRouter(initialEntries: string[]) {
	const normalizedEntries = initialEntries.map((entry) => {
		if (!entry.startsWith(routerBasename) || routerBasename === "/") {
			return entry;
		}

		const trimmedEntry = entry.slice(routerBasename.length);
		return trimmedEntry.length === 0 ? "/" : trimmedEntry;
	});

	return createMemoryRouter(appRoutes, {
		initialEntries: normalizedEntries,
	});
}

export function AppRouter({
	router = createAppRouter(),
}: {
	router?: ReturnType<typeof createAppRouter>;
}) {
	return <RouterProvider router={router} />;
}
