// Fails if a page of the built site (dist/) links to a page of the site that does not exist.
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative, sep } from 'node:path';

const root = 'dist';
const files = [];
const walk = (dir) => {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) walk(path);
    else if (name.endsWith('.html')) files.push(path);
  }
};
walk(root);

const pages = new Set(
  files.map((f) => '/' + relative(root, f).split(sep).join('/').replace(/index\.html$/, '')),
);
const skip = /^\/(_astro|pagefind)\/|\.(svg|xml|css|js|png|ico|webmanifest)$/;
const broken = [];
for (const file of files) {
  const from = '/' + relative(root, file).split(sep).join('/').replace(/index\.html$/, '');
  for (const [, href] of readFileSync(file, 'utf8').matchAll(/href="(\/[^"#?]*)/g)) {
    if (skip.test(href)) continue;
    const page = href.endsWith('/') ? href : `${href}/`;
    if (!pages.has(page)) broken.push(`${from} -> ${href}`);
  }
}
if (broken.length > 0) {
  console.error(`broken links:\n${broken.join('\n')}`);
  process.exit(1);
}
console.log(`${pages.size} pages, no broken links`);
