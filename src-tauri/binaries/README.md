# Sidecar Binaries

This directory holds platform-specific binaries that are bundled into the
Shekyl Wallet installer via Tauri's `externalBin` feature.

## Required binaries

| Binary               | Purpose                            |
|----------------------|------------------------------------|
| `shekyld`            | Shekyl blockchain daemon           |
| `shekyl-wallet-rpc`  | Wallet JSON-RPC server             |

Download compiled binaries from the
[shekyl-core releases](https://github.com/Shekyl-Foundation/shekyl-core/releases)
page, or build from source using the shekyl-core repository.

## Naming convention

Tauri requires target-triple suffixed names. After downloading or compiling,
rename the binaries to match:

```
shekyld-x86_64-unknown-linux-gnu
shekyld-aarch64-unknown-linux-gnu
shekyld-x86_64-apple-darwin
shekyld-aarch64-apple-darwin
shekyld-x86_64-pc-windows-msvc.exe

shekyl-wallet-rpc-x86_64-unknown-linux-gnu
shekyl-wallet-rpc-aarch64-unknown-linux-gnu
shekyl-wallet-rpc-x86_64-apple-darwin
shekyl-wallet-rpc-aarch64-apple-darwin
shekyl-wallet-rpc-x86_64-pc-windows-msvc.exe
```

You can check your local target triple with:

```bash
rustc -vV | grep host
```

Only the platforms you intend to build for need binaries present. Tauri
selects the matching triple at build time.

## Enabling sidecar bundling

Once binaries are placed here, add to `tauri.conf.json` under `bundle`:

```json
{
  "bundle": {
    "externalBin": [
      "binaries/shekyld",
      "binaries/shekyl-wallet-rpc"
    ]
  }
}
```

Do NOT add `externalBin` until binaries are present for every platform
in the build matrix -- missing binaries cause the build to fail.

## Git

These binaries are large and should NOT be committed to the repository.
They are listed in `.gitignore`. In CI, the release workflow should
download them from shekyl-core releases before running `tauri build`.

## Development

During development, binaries are resolved via PATH lookup. Install
`shekyld` and `shekyl-wallet-rpc` system-wide or set a custom path
in **Settings > Binary Path** within the wallet UI. No sidecar
configuration is needed for `npm run tauri dev`.
