import { useEffect } from "react";

import { CodeExample } from "../components/CodeExample";
import { ProductStatus } from "../components/ProductStatus";
import { examples } from "../content/examples";

export default function ExamplesPage() {
	useEffect(() => {
		document.title = "renderflow | Verified examples";
	}, []);

	return (
		<div className="page">
			<section className="page-intro">
				<p className="section-kicker">Examples</p>
				<h1>Verified workflows from the repository</h1>
				<p>
					These examples summarize documented Renderflow workflows without executing rendering jobs
					in the browser.
				</p>
			</section>

			<div className="example-grid">
				{examples.map((example) => (
					<article className="card example-card" key={example.identifier}>
						<div className="card-header">
							<div>
								<h2>{example.title}</h2>
								<p>{example.useCase}</p>
							</div>
							<ProductStatus status={example.status} />
						</div>
						<dl className="detail-list">
							<div>
								<dt>Input summary</dt>
								<dd>{example.inputSummary}</dd>
							</div>
							<div>
								<dt>Expected outputs</dt>
								<dd>{example.expectedOutputs.join(", ")}</dd>
							</div>
							<div>
								<dt>Applicable command</dt>
								<dd>
									<code>{example.command}</code>
								</dd>
							</div>
						</dl>
						<CodeExample code={example.configuration} title="Configuration" />
						<a href={example.documentationPath} rel="noreferrer" target="_blank">
							Source and documentation
						</a>
					</article>
				))}
			</div>
		</div>
	);
}
