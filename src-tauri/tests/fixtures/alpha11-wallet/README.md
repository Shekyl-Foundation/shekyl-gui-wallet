# An alpha.11 wallet file

`alpha11.wallet.keys` and `alpha11.wallet` are a wallet made by the
shekyl-core `v3.1.0-alpha.11` release's `shekyl-cli`, the binary in that
release's `shekyl-x86_64-linux-gnu-v3.1.0-alpha.11.tar.bz2`:

```
shekyl-cli --testnet --wallet-dir <dir> --daemon-address 127.0.0.1:1 \
  create --seed-out <file> --password-file <file> alpha11
```

The password is in `password` beside them (`fixture-password`, no newline). The wallet is on the retired alpha.11
test network, was never funded, and its seed backup was destroyed after
creation. It exists only to be refused: this release's wallet file format
postdates it (core `NETWORK_IDENTITY.md` NI-Q8), and
`lifecycle::tests::an_earlier_releases_wallet_is_refused_in_words_that_point_to_a_new_wallet`
opens it through the `open_wallet` command's body.
