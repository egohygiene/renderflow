import { render } from "@testing-library/react";

import { AppRouter, createAppMemoryRouter } from "../app/router";

export function renderRoute(initialEntry: string) {
	const router = createAppMemoryRouter([initialEntry]);
	return render(<AppRouter router={router} />);
}
