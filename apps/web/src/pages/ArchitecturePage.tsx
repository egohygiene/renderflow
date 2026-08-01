import { useEffect } from "react";

import { architectureLayers, architectureLinks } from "../content/product";

export default function ArchitecturePage() {
	useEffect(() => {
		document.title = "renderflow | Product architecture";
	}, []);

	return (
		<div className="page">
			<section className="page-intro">
				<p className="section-kicker">Architecture</p>
				<h1>Product-level architecture, not a second implementation</h1>
				<p>
					Renderflow keeps reusable rendering behavior in Rust while the web app explains how the
					core pieces fit together.
				</p>
			</section>

			<section className="section-grid" aria-labelledby="architecture-flow-title">
				<div>
					<h2 id="architecture-flow-title">High-level execution flow</h2>
					<p>
						Configuration loading, planning, execution, and caching remain the canonical runtime
						stages documented in the repository.
					</p>
				</div>
				<ol
					aria-label="Renderflow architecture flow"
					className="pipeline-diagram architecture-diagram"
				>
					<li className="pipeline-step">
						<span className="pipeline-index">01</span>
						<span>renderflow.yaml config loading and validation</span>
					</li>
					<li className="pipeline-step">
						<span className="pipeline-index">02</span>
						<span>Standard pipeline or graph-aware planning</span>
					</li>
					<li className="pipeline-step">
						<span className="pipeline-index">03</span>
						<span>Transforms, optimization, and reusable intermediates</span>
					</li>
					<li className="pipeline-step">
						<span className="pipeline-index">04</span>
						<span>Renderer or DAG executor waves</span>
					</li>
					<li className="pipeline-step">
						<span className="pipeline-index">05</span>
						<span>Output artifacts plus cache and diagnostics metadata</span>
					</li>
				</ol>
			</section>

			<section aria-labelledby="layers-title">
				<div className="section-heading">
					<div>
						<p className="section-kicker">Runtime boundaries</p>
						<h2 id="layers-title">What owns what</h2>
					</div>
					<p>
						The web application consumes public product information and links deeper into the docs;
						`renderflow-core` and `renderflow-cli` remain the implementation owners.
					</p>
				</div>
				<div className="architecture-grid">
					{architectureLayers.map((layer) => (
						<article className="card architecture-card" key={layer.title}>
							<h3>{layer.title}</h3>
							<p>{layer.description}</p>
						</article>
					))}
				</div>
			</section>

			<section className="section-grid" aria-labelledby="deep-dive-title">
				<div>
					<h2 id="deep-dive-title">Deep-dive references</h2>
					<p>
						Use the canonical architecture docs for internals, command semantics, execution plan
						structure, and plugin boundaries.
					</p>
				</div>
				<ul className="check-list">
					{architectureLinks.map((link) => (
						<li key={link.href}>
							<a href={link.href} rel="noreferrer" target="_blank">
								{link.label}
							</a>
						</li>
					))}
				</ul>
			</section>
		</div>
	);
}
