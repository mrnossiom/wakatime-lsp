# Wakatime LSP

A dead-simple LSP around wakatime-cli to send code tracking heartbeats

I made this LSP wrapper implementation around `wakatime-cli` because I wanted support for WakaTime in [Helix](https://github.com/helix-editor/helix). Yet, it's supposed to be compatible with every LSP implementations.

## Installation

<details>
  <summary>With `cargo`</summary>

While `wakatime-lsp` seems to work fine for my use case right now, I don't want to publish on `crates.io` rightaway.

```sh
cargo install --git https://github.com/mrnossiom/wakatime-lsp
```

You will also need `wakatime-cli` in path which you can download here:

- https://repology.org/project/wakatime/versions
- https://repology.org/project/wakatime-cli/versions
- or from WakaTime repository artefacts if your package manager does not bundle wakatime: https://github.com/wakatime/wakatime-cli/releases/latest

</details>

<details>
  <summary>With `nix` flakes</summary>

A `nix` package is available which means that you can use `github:mrnossiom/wakatime-lsp` as a flake identifier

- import this repository in your flake inputs

  ```nix
  {
    wakatime-lsp.url = "github:mrnossiom/wakatime-lsp";
    wakatime-lsp.inputs.nixpkgs.follows = "nixpkgs";
  }
  ```

  Add the package to your [NixOS](https://nixos.org/) or [Home Manager](https://github.com/nix-community/home-manager) packages depending on your installation.

- with `nix shell/run/profile` for imperative installation.

  e.g. `nix shell github:mrnossiom/wakatime-lsp`



</details>

## Configuration

Currently `wakatime-lsp` is not configurable cause it's more of a simple `wakatime-cli` wrapper which itself is configurable with [`$WAKATIME_HOME/.wakatime.cfg`](https://github.com/wakatime/wakatime-cli/blob/develop/USAGE.md#ini-config-file).

The only required configuration is `$WAKATIME_HOME/.wakatime.cfg` which must contain at least:

```toml
[settings]
# You can find your WakaTime api key at https://wakatime.com/settings/api-key
api_key=
```

Though it might be already filled if you've used another wakatime plugin in the past.

## Usage

### Helix

You can add this to your Helix `languages.toml` configuration. Currently, configuration does not make it possible to add global LSPs. See [new config system PR](https://github.com/helix-editor/helix/pull/9318).

```toml
[language-server.wakatime]
command = "wakatime-lsp"

[[language]]
name = "markdown"
language-servers = ["marksman", "wakatime"]

[[language]]
name = "rust"
language-servers = ["rust-analyzer", "wakatime"]

[[language]]
name = "nix"
language-servers = ["nil", "wakatime"]
```

---

Work is licensed under [`CECILL-2.1`](https://choosealicense.com/licenses/cecill-2.1/), a French OSS license that allows modification and distribution of the software while requiring the same license for derived works.
