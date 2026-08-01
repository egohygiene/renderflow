import type { FeatureStatus } from "../content/types";

const labels: Record<FeatureStatus, string> = {
	available: "Available",
	experimental: "Experimental",
	planned: "Planned",
};

export function ProductStatus({ status }: { status: FeatureStatus }) {
	return (
		<span className={`status-badge status-${status}`}>
			<span className="sr-only">Feature status:</span> {labels[status]}
		</span>
	);
}
