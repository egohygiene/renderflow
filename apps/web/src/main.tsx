import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { AppRouter } from "./app/router";
import "./styles/site.css";

const rootElement = document.getElementById("root");

if (!rootElement) {
	throw new Error("Expected #root element for the Renderflow web app.");
}

createRoot(rootElement).render(
	<StrictMode>
		<AppRouter />
	</StrictMode>,
);
