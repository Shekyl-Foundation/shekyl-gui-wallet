import { describe, expect, it } from "vitest";
import {
  CREATED_WALLET_BACKUP_UNUSABLE,
  backupEncodingFor,
  createdWalletFromWire,
  seedBackupOf,
  type CreatedWalletWire,
  type WalletHandle,
  type WalletNetwork,
} from "./wallet";

function handle(network: WalletNetwork): WalletHandle {
  return { name: "alice", capability: "FULL", network };
}

const phrase = Array.from({ length: 24 }, (_, i) => `word${i + 1}`).join(" ");
const raw = "ab".repeat(32);

function wire(network: WalletNetwork, partial: Partial<CreatedWalletWire>): CreatedWalletWire {
  return { wallet: handle(network), ...partial };
}

describe("createdWalletFromWire", () => {
  it("the network chooses the encoding", () => {
    expect(backupEncodingFor("MAINNET")).toBe("mnemonic");
    expect(backupEncodingFor("STAGENET")).toBe("mnemonic");
    expect(backupEncodingFor("TESTNET")).toBe("raw_seed_hex");
  });

  it("keeps a 24-word phrase on mainnet and stagenet", () => {
    for (const network of ["MAINNET", "STAGENET"] as const) {
      const created = createdWalletFromWire(wire(network, { mnemonic: phrase }));
      expect(created.encoding).toBe("mnemonic");
      expect(seedBackupOf(created)).toBe(phrase);
    }
  });

  it("keeps a 64-hex seed on testnet, lowercased", () => {
    const created = createdWalletFromWire(wire("TESTNET", { raw_seed_hex: raw.toUpperCase() }));
    expect(created).toEqual({ wallet: handle("TESTNET"), encoding: "raw_seed_hex", raw_seed_hex: raw });
  });

  it("refuses the arm the network does not use", () => {
    expect(() => createdWalletFromWire(wire("TESTNET", { mnemonic: phrase }))).toThrow(
      CREATED_WALLET_BACKUP_UNUSABLE,
    );
    for (const network of ["MAINNET", "STAGENET"] as const) {
      expect(() => createdWalletFromWire(wire(network, { raw_seed_hex: raw }))).toThrow(
        CREATED_WALLET_BACKUP_UNUSABLE,
      );
    }
  });

  it("refuses neither, both, and a present-but-empty arm", () => {
    expect(() => createdWalletFromWire(wire("MAINNET", {}))).toThrow(CREATED_WALLET_BACKUP_UNUSABLE);
    expect(() =>
      createdWalletFromWire(wire("MAINNET", { mnemonic: phrase, raw_seed_hex: raw })),
    ).toThrow(CREATED_WALLET_BACKUP_UNUSABLE);
    // A valid phrase beside an empty hex arm is two arms on the wire, not one.
    expect(() =>
      createdWalletFromWire(wire("MAINNET", { mnemonic: phrase, raw_seed_hex: "" })),
    ).toThrow(CREATED_WALLET_BACKUP_UNUSABLE);
    expect(() => createdWalletFromWire(wire("MAINNET", { mnemonic: "" }))).toThrow(
      CREATED_WALLET_BACKUP_UNUSABLE,
    );
    expect(() => createdWalletFromWire(wire("TESTNET", { raw_seed_hex: "   " }))).toThrow(
      CREATED_WALLET_BACKUP_UNUSABLE,
    );
  });

  it("refuses a malformed backup in the right arm", () => {
    expect(() => createdWalletFromWire(wire("MAINNET", { mnemonic: "only one" }))).toThrow(
      CREATED_WALLET_BACKUP_UNUSABLE,
    );
    expect(() => createdWalletFromWire(wire("TESTNET", { raw_seed_hex: "abcd" }))).toThrow(
      CREATED_WALLET_BACKUP_UNUSABLE,
    );
  });
});
