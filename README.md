# cacheleon.dev

My personal website, built with [Astro](https://astro.build).

The black hole on the homepage is ray traced in a WebGL shader and drawn as
ASCII; the code is in `src/blackhole/`.

`curl cacheleon.dev` plays it in the terminal. That's a small Rust server in
`curl/` that renders the loop once at startup and streams it; nginx sends
terminal user agents to it. The deploy hook and service files are in
`curl/deploy/`.

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

Pushing `main` deploys it (origin pushes to the Pi and to GitHub):

```sh
git push
```

A `post-receive` hook builds the site and copies `dist/` into nginx's web root.
A failed build leaves the live site untouched.
