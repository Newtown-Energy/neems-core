# Demo deployment (DigitalOcean App Platform)

This repo continuously deploys the public **demo API** to its own App — there is
no coordinator repo. On merge to `main`, `.github/workflows/deploy.yml` builds
and pushes `ghcr.io/newtown-energy/neems-api`, then rolls a new deployment that
re-pulls `:latest`.

The React front end is a **separate** App (`neems-react`) whose Caddy proxies
`/api` here, so the browser sees one origin and the `SameSite=Lax` session
cookie keeps working. Keep both Apps on one registrable domain — same-origin or
sibling subdomains (`api.demo.x` / `app.demo.x`), never fully cross-site.

## One-time setup

1. **Make the `neems-api` GHCR package public** (GitHub → org **Packages** →
   `neems-api` → **Package settings** → change visibility → **Public**). The
   image is just compiled binaries of this public repo, so App Platform can then
   pull it with **no credential — no PAT to create or rotate.** The package only
   appears after the first image push (first merge to `main`).
   *(Private alternative: keep it private and set `registry_credentials` in
   `.do/app.yaml` — see the comment there. A PAT expires; public access does not.)*
2. Create the App straight from the committed spec (it holds no secrets):
   ```bash
   doctl auth init          # paste your DigitalOcean API token
   doctl apps create --spec .do/app.yaml
   doctl apps list          # note the App id + the *.ondigitalocean.app URL
   ```
3. Set `NEEMS_DEFAULT_PASSWORD` as a SECRET on the App (dashboard) before you
   share the URL. Because the SQLite DB is ephemeral, the superadmin is
   recreated from `NEEMS_DEFAULT_EMAIL`/`NEEMS_DEFAULT_PASSWORD` on every deploy,
   so the value takes effect on the next deploy.
4. Add repo secrets so CI can deploy: `DIGITALOCEAN_ACCESS_TOKEN`, `DIGITALOCEAN_APP_ID`.

After this, every merge to `main` publishes an image and redeploys the App.

## Notes

- Routine CD uses `doctl apps create-deployment`, which redeploys with the App's
  **stored** config — so the committed spec's placeholder password is never
  applied by CI (your dashboard-set secret stands).
- To change the spec itself, edit `.do/app.yaml` and run
  `doctl apps update <id> --spec .do/app.yaml` manually (re-set secrets after).
- `instance_size_slug` in the spec is a starting guess; confirm a current value
  with `doctl apps tier instance-size list`.
