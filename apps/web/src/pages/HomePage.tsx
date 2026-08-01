import { useEffect } from "react";
import { Link } from "react-router-dom";

import { CodeExample } from "../components/CodeExample";
import { PipelineDiagram } from "../components/PipelineDiagram";
import { ProductStatus } from "../components/ProductStatus";
import { capabilities } from "../content/capabilities";
import {
	architectureLayers,
	architectureLinks,
	installationMethods,
	productIdentity,
	whyRenderflow,
} from "../content/product";

export default function HomePage() {
	useEffect(() => {
		document.title = "renderflow | Spec-driven document rendering engine";
	}, []);

	return (
		<div className="page page-home">
			<section className="hero-panel">
				<div className="hero-copy">
					<p className="eyebrow">✨ {productIdentity.name}</p>
					<h1>{productIdentity.tagline}</h1>
					<p className="hero-summary">{productIdentity.summary}</p>
					<div className="hero-actions">
						<a
							className="button button-primary"
							href={productIdentity.documentationUrl}
							rel="noreferrer"
							target="_blank"
						>
							Get started
						</a>
						<Link className="button button-secondary" to="/examples">
							View examples
						</Link>
						<Link className="button button-ghost" to="/architecture">
							Architecture
						</Link>
						<a
							className="button button-ghost"
							href={productIdentity.repositoryUrl}
							rel="noreferrer"
							target="_blank"
						>
							GitHub repository
						</a>
					</div>
				</div>
				<div aria-hidden="true" className="hero-orbit">
					<span>markdown</span>
					<span>yaml</span>
					<span>graph</span>
					<span>pdf</span>
					<span>html</span>
				</div>
			</section>

			<section className="section-grid" aria-labelledby="pipeline-title">
				<div>
					<p className="section-kicker">Product pipeline</p>
					<h2 id="pipeline-title">Readable before it runs</h2>
					<p>
						Renderflow keeps planning visible: source files, config, graph selection, transforms,
						and output artifacts are all explicit.
					</p>
				</div>
				<PipelineDiagram />
			</section>

			<section aria-labelledby="capabilities-title">
				<div className="section-heading">
					<div>
						<p className="section-kicker">Capability overview</p>
						<h2 id="capabilities-title">Verified from the repository</h2>
					</div>
					<p>
						Each capability is driven from typed content with evidence links back to the current
						Renderflow docs or source tree.
					</p>
				</div>
				<div className="capability-grid">
					{capabilities.map((capability) => (
						<article className="card capability-card" key={capability.identifier}>
							<div className="card-header">
								<h3>{capability.title}</h3>
								<ProductStatus status={capability.status} />
							</div>
							<p>{capability.description}</p>
							<div className="card-meta">
								<span>{capability.evidence}</span>
								{capability.documentationPath ? (
									<a href={capability.documentationPath} rel="noreferrer" target="_blank">
										Evidence
									</a>
								) : null}
							</div>
						</article>
					))}
				</div>
			</section>

			<section className="section-grid" aria-labelledby="config-title">
				<div>
					<p className="section-kicker">Configuration example</p>
					<h2 id="config-title">A real `renderflow.yaml` shape</h2>
					<p>
						This example is taken from the documented configuration reference and matches supported
						keys in the current codebase.
					</p>
				</div>
				<CodeExample
					caption={
						<a href={productIdentity.configurationSource} rel="noreferrer" target="_blank">
							Source reference
						</a>
					}
					code={productIdentity.configurationExample}
					title="docs/user-guide/configuration.md"
				/>
			</section>

			<section className="section-grid" aria-labelledby="why-title">
				<div>
					<p className="section-kicker">Why Renderflow</p>
					<h2 id="why-title">Designed for reproducible publishing workflows</h2>
				</div>
				<ul className="check-list">
					{whyRenderflow.map((reason) => (
						<li key={reason}>{reason}</li>
					))}
				</ul>
			</section>

			<section aria-labelledby="architecture-preview-title">
				<div className="section-heading">
					<div>
						<p className="section-kicker">Architecture preview</p>
						<h2 id="architecture-preview-title">Keep the engine in Rust</h2>
					</div>
					<p>
						The site explains the product and links to the canonical technical docs; it never
						becomes a second implementation of the rendering engine.
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
				<ul className="inline-links">
					{architectureLinks.map((link) => (
						<li key={link.href}>
							<a href={link.href} rel="noreferrer" target="_blank">
								{link.label}
							</a>
						</li>
					))}
				</ul>
			</section>

			<section className="section-grid" aria-labelledby="installation-title">
				<div>
					<p className="section-kicker">Installation</p>
					<h2 id="installation-title">Only documented installation paths</h2>
					<p>
						Commands below are limited to installation methods already documented in this
						repository, with planned distribution targets called out explicitly.
					</p>
				</div>
				<div className="installation-grid">
					{installationMethods.map((method) => (
						<article className="card" key={method.identifier}>
							<div className="card-header">
								<h3>{method.title}</h3>
								<ProductStatus status={method.status} />
							</div>
							<p>{method.notes}</p>
							{method.command ? (
								<CodeExample code={method.command} title="Install command" />
							) : null}
							<a href={method.documentationPath} rel="noreferrer" target="_blank">
								Installation details
							</a>
						</article>
					))}
				</div>
			</section>

			<section className="section-grid" aria-labelledby="ecosystem-title">
				<div>
					<p className="section-kicker">Ecosystem context</p>
					<h2 id="ecosystem-title">Part of Ego Hygiene</h2>
					<p>
						Renderflow owns its product explanation, examples, and release-aligned visuals here
						while the broader Ego Hygiene website handles cross-product navigation and the
						`/renderflow/` gateway path.
					</p>
				</div>
				<div className="card ecosystem-card">
					<p>
						Visit{" "}
						<a href={productIdentity.ecosystemUrl} rel="noreferrer" target="_blank">
							egohygiene.io
						</a>{" "}
						for the wider ecosystem and use this standalone app directly during local development at{" "}
						<code>http://localhost:5173/renderflow/</code>.
					</p>
				</div>
			</section>
		</div>
	);
}
