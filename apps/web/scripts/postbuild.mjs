import { copyFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const distDirectory = resolve(fileURLToPath(new URL(".", import.meta.url)), "..", "dist");
copyFileSync(resolve(distDirectory, "index.html"), resolve(distDirectory, "404.html"));
