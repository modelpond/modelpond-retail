# Model Pond Retail

> [!IMPORTANT]
> This source code is provided initially to enable verifying its
> functionality, and the provenance of built assets operating in the
> [Model Pond](https://www.modelpond.eu/) production environment. It is
> shared source under the [PolyForm Strict licence](LICENCE.md), not free
> software. This means you may not distribute the software or operate it for
> commercial purposes, modify it, or contribute changes to it.

```
▓▓▓███▓▓▒
▓█████████▒░
▓███████████▒░           ▒▓▓▒▒▒▒▒▒░░
 ░░░░░░▒▒▓████▓▒░      ▒▓█████▓▓▓▓▓▓▒
          ░▒██████▓▓▒░▓▓▓▓████▓▒▓▓▓▓▓▒░
             ▒▓█████▓▓▓▓▓▓▓▓▓░  ▒▓▓▓▓▓▓▒░
               ░▓▓▓▓▓▓▓▓▓▓▓░     ░▓▓▓▓▓▓▓▒░
░░░░░░░░▒▒▒▒▓▓▓▓▓▓▓▓▓▓▓▓▒░         ▒▓▓▓▓█▓█▓▒░░░░░░░░
▓▓▓█████▓███████▓▓▓▓▒▒░              ▒▓▓█████▓▓▓▓▓▓▓▓▒░
▓██████████▓▓▓▓▓▓▓▒░░                 ░▒▓█████▓▓▓▓▓▓▓▓▒
▒▓▓▓▓▓▓▓▓▓▓▓█▓▓▓▓▓▓▓▓▓▒▒░           ▒▓██████▓▓▓▓▓▓▓▓▒░
             ░░▒▓▓▓▓▓▓▓▓▓▒░       ▒▓▓▓▓▓▓▓▒░░
              ░▒▓██▓▓▓▓▓▓▓▓▓░   ░▓▓▓▓▓▓▓▒
           ░▒▓██████▓▓▓▓▓████▓░▒▓▓▓▓▓▓▒░
       ░▒▒▓████▓▒░    ▒▓██████▓▓▓▓▓▓▒░
▓███████████▓▒          ▒▓██▓▓▓▓▓▓▒░
██████████▓░
▓█████▓▓▒░
```

The retail site is generated entirely by `build.rs`.

```sh
cargo build
python3 -m http.server 8000 --directory target/html
```

Templates live in `content/templates`, static files in `content/static`, and the
catalog-backed MiniJinja context comes directly from `mp_catalog`.

`cargo build` emits:

```text
target/html/
  index.html
  instruct/index.html
  gateway/index.html
  code/index.html
  batch/index.html
  catalogue/index.html
  catalogue/provider/<provider-id>/index.html
  catalogue/models/<model-id>/index.html
  privacy/index.html
  static/
```

`make upload` mirrors `target/html/` to `public_html`, deleting remote files
that no longer exist in the generated output. `make dry-run` previews it.
