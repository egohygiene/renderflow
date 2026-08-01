import { productIdentity } from "../content/product";

export function Footer() {
	return (
		<footer className="site-footer">
			<div>
				<strong>{productIdentity.name}</strong>
				<p>
					Renderflow ships its own product site, examples, and release-aligned product content from
					this repository.
				</p>
			</div>
			<ul className="footer-links">
				<li>
					<a href={productIdentity.documentationUrl} rel="noreferrer" target="_blank">
						Technical documentation
					</a>
				</li>
				<li>
					<a href={productIdentity.repositoryUrl} rel="noreferrer" target="_blank">
						Source repository
					</a>
				</li>
				<li>
					<a href={productIdentity.ecosystemUrl} rel="noreferrer" target="_blank">
						Ego Hygiene ecosystem
					</a>
				</li>
			</ul>
		</footer>
	);
}
