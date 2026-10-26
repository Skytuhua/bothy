# Bothy documentation site

The site at <https://bothysync.dev>, built with [Astro Starlight](https://starlight.astro.build).
Pages are Markdown in `src/content/docs/`; the sidebar is in `astro.config.mjs`.

```sh
npm ci
npm run dev            # http://localhost:4321
npm run build          # static output in dist/
npm run check-links    # after a build: fails on links to pages that do not exist
```

CI builds the site and checks links on every pull request, and deploys `main` to Cloudflare
Pages (project `bothy`) with `SITE_URL=https://bothysync.dev`.
