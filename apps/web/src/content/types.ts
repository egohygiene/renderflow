export type FeatureStatus = "available" | "experimental" | "planned";

export interface ExternalLink {
	readonly href: string;
	readonly label: string;
}

export interface RenderflowCapability {
	readonly identifier: string;
	readonly title: string;
	readonly description: string;
	readonly status: FeatureStatus;
	readonly documentationPath?: string;
	readonly evidence: string;
}

export interface RenderflowExample {
	readonly identifier: string;
	readonly title: string;
	readonly useCase: string;
	readonly inputSummary: string;
	readonly configuration: string;
	readonly expectedOutputs: readonly string[];
	readonly command: string;
	readonly status: FeatureStatus;
	readonly documentationPath: string;
}

export interface RenderflowNavigationItem {
	readonly href: string;
	readonly label: string;
}

export interface InstallationMethod {
	readonly identifier: string;
	readonly title: string;
	readonly command?: string;
	readonly notes: string;
	readonly status: FeatureStatus;
	readonly documentationPath: string;
}
