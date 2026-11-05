import { describe, expect, it } from "vitest";
import {
  CREATED_WALLET_BACKUP_UNUSABLE,
  createdWalletFromWire,
  type CreatedWalletWire,
  type WalletHandle,
  type WalletNetwork,
} from "./wallet";

function handle(network: WalletNetwork): WalletHandle {
  return { name: "alice", capability: "FULL", network };
}

const phrase = Array.from({ length: 24 }, (_, i) => `word${i + 1}`).join(" ");
const raw = "ab".repeat(32);
const NETWORKS = ["MAINNET", "TESTNET", "STAGENET"] as const;

function wire(network: WalletNetwork, partial: Partial<CreatedWalletWire>): CreatedWalletWire {
  return { wallet: handle(network), ...partial };
}

describe("createdWalletFromWire", () => {
  it("keeps the 24-word phrase on every network", () => {
    for (const network of NETWORKS) {
      const created = createdWalletFromWire(wire(network, { mnemonic: ` ${phrase} ` }));
      expect(created).toEqual({ wallet: handle(network), mnemonic: phrase });
    }
  });

  it("refuses a hex seed in place of the phrase, testnet included", () => {
    for (const network of NETWORKS) {
      expect(() => createdWalletFromWire(wire(network, { mnemonic: raw }))).toThrow(
        CREATED_WALLET_BACKUP_UNUSABLE,
      );
    }
  });

  it("refuses a missing, empty or blank phrase", () => {
    for (const network of NETWORKS) {
      expect(() => createdWalletFromWire(wire(network, {}))).toThrow(
        CREATED_WALLET_BACKUP_UNUSABLE,
      );
      for (const mnemonic of ["", "   "]) {
        expect(() => createdWalletFromWire(wire(network, { mnemonic }))).toThrow(
          CREATED_WALLET_BACKUP_UNUSABLE,
        );
      }
    }
  });

  it("refuses a phrase that is not 24 words", () => {
    const twelve = phrase.split(" ").slice(0, 12).join(" ");
    for (const mnemonic of ["only one", twelve, `${phrase} extra`]) {
      expect(() => createdWalletFromWire(wire("MAINNET", { mnemonic }))).toThrow(
        CREATED_WALLET_BACKUP_UNUSABLE,
      );
    }
  });
});
