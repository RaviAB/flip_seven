import { createHash } from "node:crypto";
import { cp, readFile, readdir, writeFile } from "node:fs/promises";
import path from "node:path";

const output = process.env.TRUNK_STAGING_DIR;
if (!output) throw new Error("Run this script through the Trunk post-build hook.");
await cp("LICENSE", path.join(output, "LICENSE.txt"));

async function filesIn(directory, prefix = "") {
  const files = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const name = prefix + entry.name;
    if (entry.isDirectory()) files.push(...await filesIn(path.join(directory, entry.name), name + "/"));
    else files.push(name);
  }
  return files.sort();
}

const files = (await filesIn(output)).filter(name => name !== "sw.js");
const hash = createHash("sha256");
for (const file of files) {
  hash.update(file);
  hash.update(await readFile(path.join(output, file)));
}
const source = await readFile("web/sw.js", "utf8");
await writeFile(path.join(output, "sw.js"), source
  .replace("__BUILD_HASH__", hash.digest("hex").slice(0, 16))
  .replace("__PRECACHE__", JSON.stringify(files.map(file => "./" + file))));
await writeFile(path.join(output, ".nojekyll"), "");
