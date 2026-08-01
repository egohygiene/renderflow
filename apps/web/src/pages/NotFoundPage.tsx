import { useEffect } from "react";
import { Link } from "react-router-dom";

import { productIdentity } from "../content/product";

export default function NotFoundPage() {
	useEffect(() => {
		document.title = "renderflow | Page not found";
	}, []);

	return (
		<div className="page not-found-page">
			<section className="page-intro">
				<p className="section-kicker">Not found</p>
				<h1>That Renderflow page does not exist</h1>
				<p>Try the product overview, the verified examples, or the wider Ego Hygiene ecosystem.</p>
			</section>
			<div className="hero-actions">
				<Link className="button button-primary" to="/">
					Back to renderflow
				</Link>
				<Link className="button button-secondary" to="/examples">
					Browse examples
				</Link>
				<a
					className="button button-ghost"
					href={productIdentity.ecosystemUrl}
					rel="noreferrer"
					target="_blank"
				>
					Visit egohygiene.io
				</a>
			</div>
		</div>
	);
}
