# cacheleon.dev

My personal website, built with [Astro](https://astro.build).

## Run it locally

```sh
npm install
npm run dev
```

## Deploy

Pushing `main` to the Pi deploys it:

```sh
git push pi main
```

A `post-receive` hook builds the site and copies `dist/` into nginx's web root.
A failed build leaves the live site untouched.
