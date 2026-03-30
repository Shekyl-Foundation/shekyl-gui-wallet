# Sidecar Binaries

This directory holds platform-specific binaries that are bundled into the
Shekyl Wallet installer via Tauri's `externalBin` feature.

## Required binaries

| Binary               | Purpose                            |
|----------------------|------------------------------------|
| `shekyld`            | Shekyl blockchain daemon           |
| `shekyl-wallet-rpc`  | Wallet JSON-RPC server             |

## Naming convention

Tauri requires target-triple suffixed names:

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

## Enabling sidecar bundling

Once binaries are placed here, add to `tauri.conf.json`:

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

Tauri automatically selects the correct platform binary at build time.

## Development

During development, binaries are resolved via PATH lookup. Install
`shekyld` and `shekyl-wallet-rpc` system-wide or set a custom path
in **Settings > Binary Path** within the wallet UI.
