import { NavLink } from "react-router-dom";

import { navigationItems } from "../content/navigation";
import { productIdentity } from "../content/product";

export function Header() {
	return (
		<header className="site-header">
			<div className="brand-lockup">
				<NavLink className="brand-mark" to="/">
					<span className="brand-symbol" aria-hidden="true">
						✨
					</span>
					<span>{productIdentity.name}</span>
				</NavLink>
				<p>{productIdentity.tagline}</p>
			</div>

			<nav aria-label="Primary navigation">
				<ul className="nav-list">
					{navigationItems.map((item) => (
						<li key={item.href}>
							<NavLink
								className={({ isActive }) => (isActive ? "nav-link nav-link-active" : "nav-link")}
								end={item.href === "/"}
								to={item.href}
							>
								{item.label}
							</NavLink>
						</li>
					))}
				</ul>
			</nav>

			<div className="header-actions">
				<a
					className="button button-ghost"
					href={productIdentity.documentationUrl}
					rel="noreferrer"
					target="_blank"
				>
					Documentation
				</a>
				<a
					className="button button-primary"
					href={productIdentity.repositoryUrl}
					rel="noreferrer"
					target="_blank"
				>
					GitHub
				</a>
			</div>
		</header>
	);
}
