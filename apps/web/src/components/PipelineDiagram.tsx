import { productPipeline } from "../content/product";

export function PipelineDiagram() {
	return (
		<ol aria-label="Renderflow product pipeline" className="pipeline-diagram">
			{productPipeline.map((step, index) => (
				<li key={step} className="pipeline-step">
					<span className="pipeline-index">0{index + 1}</span>
					<span>{step}</span>
				</li>
			))}
		</ol>
	);
}
