import { describe, expect, it } from "vitest";
import {
  CREATED_WALLET_BACKUP_UNUSABLE,
  createdWalletFromWire,
  seedBackupOf,
  type CreatedWalletWire,
  type WalletHandle,
} from "./wallet";

const wallet: WalletHandle = {
  name: "alice",
  capability: "FULL",
  network: "TESTNET",
};

const phrase = Array.from({ length: 24 }, (_, i) => `word${i + 1}`).join(" ");
const raw = "ab".repeat(32);

function wire(partial: Partial<CreatedWalletWire>): CreatedWalletWire {
  return { wallet, ...partial };
}

describe("createdWalletFromWire", () => {
  it("keeps a 24-word phrase and drops a missing raw seed", () => {
    const created = createdWalletFromWire(wire({ mnemonic: phrase }));
    expect(created.encoding).toBe("mnemonic");
    if (created.encoding !== "mnemonic") return;
    expect(seedBackupOf(created)).toBe(phrase);
  });

  it("keeps a 64-hex testnet seed, lowercased", () => {
    const created = createdWalletFromWire(wire({ raw_seed_hex: raw.toUpperCase() }));
    expect(created).toEqual({
      wallet,
      encoding: "raw_seed_hex",
      raw_seed_hex: raw,
    });
  });

  it("refuses neither backup, both backups, and a malformed seed", () => {
    expect(() => createdWalletFromWire(wire({}))).toThrow(CREATED_WALLET_BACKUP_UNUSABLE);
    expect(() => createdWalletFromWire(wire({ mnemonic: phrase, raw_seed_hex: raw }))).toThrow(
      CREATED_WALLET_BACKUP_UNUSABLE,
    );
    expect(() => createdWalletFromWire(wire({ mnemonic: "only one" }))).toThrow(
      CREATED_WALLET_BACKUP_UNUSABLE,
    );
    expect(() => createdWalletFromWire(wire({ raw_seed_hex: "abcd" }))).toThrow(
      CREATED_WALLET_BACKUP_UNUSABLE,
    );
    expect(() => createdWalletFromWire(wire({ mnemonic: "   ", raw_seed_hex: "" }))).toThrow(
      CREATED_WALLET_BACKUP_UNUSABLE,
    );
  });
});
