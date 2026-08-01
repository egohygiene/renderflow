import type { ReactNode } from "react";

export function CodeExample({
	title,
	code,
	caption,
}: {
	title: string;
	code: string;
	caption?: ReactNode;
}) {
	return (
		<figure className="code-example">
			<figcaption>
				<span>{title}</span>
				{caption ? <span className="code-example-caption">{caption}</span> : null}
			</figcaption>
			<pre>
				<code>{code}</code>
			</pre>
		</figure>
	);
}
