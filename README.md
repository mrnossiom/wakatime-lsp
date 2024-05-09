# Wakatime LSP ![Nix Powered](https://img.shields.io/badge/Nix-Powered-blue?logo=nixos) [![Cachix Cache](https://img.shields.io/badge/cachix-mrnossiom-blue.svg)](https://mrnossiom.cachix.org)

<p align="center"><strong>
A dead-simple LSP around <code>wakatime-cli</code> to send code tracking heartbeats
</strong></p>

I made this LSP wrapper implementation around `wakatime-cli` because I wanted support for WakaTime in [Helix](https://github.com/helix-editor/helix). That said, it's should be compatible with every LSP implementations.

## Installation

<details>
  <summary>With <code>cargo</code></summary>

Install from repository with cargo:

```sh
cargo install --git https://github.com/mrnossiom/wakatime-lsp
```

While `wakatime-lsp` seems to work fine for my use case right now, I don't want to publish on `crates.io` rightaway.

You will also need `wakatime-cli` in path which you can download:

- With you prefered package manager at [`wakatime` repology](https://repology.org/project/wakatime/versions) or [`wakatime-cli` repology](https://repology.org/project/wakatime-cli/versions)
- or from [WakaTime repository artefacts](https://github.com/wakatime/wakatime-cli/releases/latest)

</details>

<details>
  <summary>With <code>nix</code> flakes</summary>

A `flake.nix` is available which means that you can use `github:mrnossiom/wakatime-lsp` as a flake identifier, so you can.

- import this repository in your flake inputs

  ```nix
  {
    wakatime-lsp.url = "github:mrnossiom/wakatime-lsp";
    wakatime-lsp.inputs.nixpkgs.follows = "nixpkgs";
  }
  ```

  Add the package to your [NixOS](https://nixos.org/) or [Home Manager](https://github.com/nix-community/home-manager) packages depending on your installation.

- use with `nix shell`/`nix run` for temporary testing

  e.g. `nix shell github:mrnossiom/wakatime-lsp`

- use with `nix profile` for imperative installation

  e.g. `nix profile install github:mrnossiom/wakatime-lsp`

Package is reachable through `packages.${system}.default` or `packages.${system}.wakatime-lsp`.

</details>

## Configuration

Currently `wakatime-lsp` is not configurable cause it's more of a simple `wakatime-cli` wrapper which itself is configurable with [`$WAKATIME_HOME/.wakatime.cfg`](https://github.com/wakatime/wakatime-cli/blob/develop/USAGE.md#ini-config-file).

Required configuration is to set your [WakaTime api key](https://wakatime.com/settings/api-key) in `$WAKATIME_HOME/.wakatime.cfg`, like so:

```ini
[settings]
api_key=waka_xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
```

It might be already filled if you've used another wakatime plugin in the past.

## Usage

### Helix

You can add `wakatime-lsp` to your `languages.toml` configuration. Currently, it's possible to add global LSPs. You can add `wakatime` for significant languages. See [new Helix config system PR](https://github.com/helix-editor/helix/pull/9318).

e.g.

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
