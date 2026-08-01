import { Outlet } from "react-router-dom";

import { Footer } from "../components/Footer";
import { Header } from "../components/Header";
import { SkipLink } from "../components/SkipLink";

export function App() {
	return (
		<>
			<SkipLink />
			<div className="site-shell">
				<Header />
				<main id="main-content">
					<Outlet />
				</main>
				<Footer />
			</div>
		</>
	);
}
