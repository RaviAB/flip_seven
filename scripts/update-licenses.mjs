import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync } from "node:fs";
import path from "node:path";

execFileSync("cargo", ["about", "generate", "--no-default-features", "--locked", "--fail",
  "-o", "web/third-party-licenses.html", "web/licenses.hbs"], { stdio: "inherit" });
const metadata = JSON.parse(execFileSync("cargo", ["metadata", "--format-version=1", "--locked",
  "--no-default-features", "--filter-platform", "wasm32-unknown-unknown"], { encoding: "utf8" }));
const fonts = metadata.packages.find(pkg => pkg.name === "epaint_default_fonts");
if (!fonts) throw new Error("Bundled font package not found");
const destination = "web/assets/font-licenses";
mkdirSync(destination, { recursive: true });
for (const file of ["UFL.txt", "OFL.txt", "Hack-Regular.txt", "emoji-icon-font-mit-license.txt"]) {
  copyFileSync(path.join(path.dirname(fonts.manifest_path), "fonts", file), path.join(destination, file));
}
