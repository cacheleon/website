# cacheleon.dev

My personal website, built with [Astro](https://astro.build).

The black hole on the homepage is ray traced in a WebGL shader and drawn as
ASCII; the code is in `src/blackhole/`.

## Run it locally

```sh
npm install
npm run dev
```

## Format the code

```sh
npm run format
```

## Deploy

Pushing `main` deploys it (origin pushes to the Pi and to GitLab):

```sh
git push
```

A `post-receive` hook builds the site and copies `dist/` into nginx's web root.
A failed build leaves the live site untouched.
