# Shekyl Wallet User Guide

Welcome to the Shekyl Wallet! This guide walks you through everything you
need to know to use the wallet, even if you've never touched a cryptocurrency
wallet before. We'll explain the "why" along with the "how," so you can feel
confident about what's happening with your money at every step.

---

## Table of Contents

1. [What Is Shekyl?](#what-is-shekyl)
2. [Before You Start: The Daemon](#before-you-start-the-daemon)
3. [Installing the Wallet](#installing-the-wallet)
4. [Your First Launch](#your-first-launch)
5. [The Dashboard](#the-dashboard)
6. [Creating a Wallet](#creating-a-wallet)
7. [Your Mnemonic Seed -- Read This Carefully](#your-mnemonic-seed--read-this-carefully)
8. [Receiving SKL](#receiving-skl)
9. [Sending SKL](#sending-skl)
10. [Transaction History](#transaction-history)
11. [Mining: Earning SKL with Your Computer](#mining-earning-skl-with-your-computer)
12. [Staking: Earning Yield While Strengthening Privacy](#staking-earning-yield-while-strengthening-privacy)
13. [Chain Health: What's Happening on the Network](#chain-health-whats-happening-on-the-network)
14. [Settings and Network Switching](#settings-and-network-switching)
15. [Understanding Post-Quantum Security](#understanding-post-quantum-security)
16. [Staying Safe](#staying-safe)
17. [Glossary](#glossary)
18. [Getting More Help](#getting-more-help)

---

## What Is Shekyl?

Shekyl (ticker: **SKL**) is a cryptocurrency -- digital money that exists on a
decentralized network of computers rather than in a bank. What makes Shekyl
special:

- **Privacy by default.** When you send SKL, your transaction is mixed in with
  decoy transactions so nobody can tell who sent what to whom. This happens
  automatically -- you don't need to do anything special.

- **Post-quantum security.** Most cryptocurrencies use math that future quantum
  computers could eventually break. Shekyl already protects every transaction
  with two layers of cryptography: one classical and one quantum-resistant.
  It's like having two locks on your front door, each requiring a completely
  different kind of key.

- **Fair mining.** Shekyl uses an algorithm called RandomX that's designed to
  run on ordinary CPUs -- the processor already in your laptop or desktop.
  You don't need to buy expensive specialized hardware to participate.

---

## Before You Start: The Daemon

Here's the one thing that catches most newcomers: the Shekyl Wallet doesn't
contain the entire blockchain itself. Instead, it talks to a separate program
called the **daemon** (`shekyld`), which does the heavy lifting of
connecting to the network, downloading blocks, and verifying transactions.

Think of it this way:

- The **daemon** is like the engine of a car. It does all the hard work.
- The **wallet** is the dashboard and steering wheel. It gives you a friendly
  way to control everything.

You need the daemon running before the wallet can do anything useful. If you
haven't set up the daemon yet, visit the
[shekyl-core documentation](https://github.com/Shekyl-Foundation/shekyl-core)
for instructions. The short version:

1. Download `shekyld` for your platform.
2. Run it. It will start downloading the blockchain (this takes a while the
   first time).
3. Leave it running in the background.

The wallet will automatically connect to the daemon on your local machine. You
can check the connection status in the top-right corner of the wallet -- a
green indicator means you're connected.

---

## Installing the Wallet

Detailed installation instructions for every platform (Linux, Windows, macOS)
are in the separate [INSTALLATION.md](INSTALLATION.md) file. The short version:

1. Go to the
   [Releases page](https://github.com/Shekyl-Foundation/shekyl-gui-wallet/releases).
2. Download the file for your operating system.
3. Run the installer (Windows/macOS) or make the AppImage executable (Linux).

That's it -- no terminal commands required to get started.

---

## Your First Launch

When you open the wallet for the first time, it does three things automatically:

1. **Starts the wallet service** -- the wallet launches `shekyl-wallet-rpc` in
   the background. This is the process that manages your wallet files and talks
   to the daemon. You'll briefly see a loading screen with the Shekyl logo.

2. **Scans for existing wallets** -- the wallet checks your default data
   directory for `.keys` files:
   - **Linux**: `~/.shekyl/wallets/`
   - **macOS**: `~/Library/Application Support/shekyl/wallets/`
   - **Windows**: `%APPDATA%\shekyl\wallets\`

3. **Routes you to the right screen**:
   - **No wallet found**: You'll see the **Welcome** screen with two options:
     "Create New Wallet" or "Import Existing Wallet."
   - **One wallet found**: You'll see the **Unlock** screen asking for your
     password.
   - **Multiple wallets found**: You'll see a wallet picker followed by the
     Unlock screen.

The main Dashboard with sidebar, send, receive, staking, and other features
only becomes accessible after a wallet is successfully opened.

**If the wallet service fails to start**, you'll see an error message with
instructions. Make sure `shekyl-wallet-rpc` is installed and accessible on
your system PATH. During development or custom installations, you can configure
the binary path in Settings.

---

## The Dashboard

The Dashboard is your home screen. Here's what you'll find:

- **Balance Card** -- Shows your total SKL balance, how much is unlocked
  (spendable right now), and how much is locked in staking.

- **Quick Actions** -- Four buttons that take you to the most common tasks:
  Send, Receive, Staking, and History.

- **Chain Health** (compact view) -- A snapshot of the network's vital signs:
  how many coins are staked, the emission rate, the burn rate, and more. This
  only appears when you're connected to the daemon.

---

## Creating a Wallet

If this is your first time, you'll see the Welcome screen. Click
**Create New Wallet** to start the setup wizard:

1. **Name and password.** Choose a name (just a label for you -- it doesn't
   appear on the blockchain) and a strong password. A strength indicator helps
   you pick something secure. This password encrypts your wallet file. If
   someone steals your computer, they can't open the wallet without it.

2. **Seed phrase.** The wallet generates a **25-word mnemonic seed** and
   displays it in a numbered grid. This is the most important thing you'll
   encounter. Read the next section carefully. You can also copy it to your
   clipboard, but **write it down on paper immediately**.

3. **Seed confirmation.** To make sure you actually saved your seed, the wallet
   asks you to enter 4 randomly chosen words (e.g., "Enter word #3, #8, #17,
   #22"). This prevents accidentally clicking through without saving.

4. **Done.** Your wallet is created and ready to use. You'll see your address
   and a confirmation that it's protected by hybrid PQC signatures. Click
   "Open Wallet" to enter the main app.

Your wallet is automatically a **v3 wallet** with full post-quantum key
material (Ed25519 + ML-DSA-65). No extra steps are needed for PQC protection.

---

## Your Mnemonic Seed -- Read This Carefully

When your wallet is created, you'll be shown 25 words. These words *are* your
wallet. Anyone who has these words can access your funds from any computer,
even years from now. This is both powerful and dangerous.

**What you MUST do:**

- Write the words down on paper. Yes, physical paper.
- Store that paper somewhere safe -- a lockbox, a safe, a safety deposit box.
- Consider making a second copy and storing it in a different location.

**What you must NEVER do:**

- Don't take a screenshot.
- Don't save them in a text file, email, or cloud drive.
- Don't share them with anyone. Ever. No legitimate support person will ever
  ask for your seed words.

**Why this matters:** If your computer breaks, gets stolen, or the wallet
software is deleted, your seed words are the *only* way to recover your funds.
No company, no foundation, no developer can recover them for you. This is
both the freedom and the responsibility of owning cryptocurrency.

If you lose your seed words and your wallet file, your SKL is gone permanently.
There is no "forgot password" button.

---

## Receiving SKL

Getting SKL sent to you is the easiest part:

1. Click **Receive** in the sidebar.
2. You'll see your **address** -- a long string starting with `SKL1`. This is
   like your bank account number, except it's safe to share publicly. Nobody
   can steal funds by knowing your address alone.
3. Click the **copy button** next to the address to copy it to your clipboard.
4. Share this address with the person or service sending you SKL.

A QR code will also be displayed, which is handy if someone wants to scan it
from their phone.

**Privacy note:** Shekyl uses **stealth addresses**, which means every
transaction to you creates a unique one-time destination on the blockchain.
Even if you give the same address to two different people, an outside observer
can't tell that both payments went to you.

---

## Sending SKL

1. Click **Send** in the sidebar.
2. Paste the **recipient's address** (starts with `SKL1`) into the
   "Recipient Address" field.
3. Enter the **amount** in SKL.
4. Click **Send**.

The wallet will automatically:

- Select which of your coins to spend (picking from your unlocked balance).
- Mix your transaction with decoy inputs (ring signatures) so nobody can tell
  which coins were really yours.
- Sign the transaction with both classical *and* quantum-resistant signatures.
- Broadcast it to the network via the daemon.

**Fees:** A small transaction fee is deducted automatically. The fee goes to
miners who include your transaction in a block.

**Confirmation:** Your transaction is considered confirmed after it's included
in a block. The recipient will see it as "pending" until enough blocks have
been mined on top of it (typically 10 confirmations for full assurance).

---

## Transaction History

Click **Transactions** in the sidebar to see a list of all your past
transactions -- both sent and received.

Each entry shows:

- The transaction hash (a unique ID).
- The amount and direction (incoming or outgoing).
- The block height it was confirmed in.
- The timestamp.
- Whether it's fully confirmed.

---

## Mining: Earning SKL with Your Computer

Mining is how new SKL enters circulation, and it's how the network stays
secure. When you mine, your computer works on mathematical puzzles. If it
solves one, you earn the **block reward** -- newly created SKL -- plus any
transaction fees from that block.

### How to start mining

1. Click **Mining** in the sidebar.
2. Enter your **SKL address** (the same one from the Receive page). This is
   where your mining rewards will be sent.
3. Choose how many **CPU threads** to use. The slider goes from 1 to however
   many your computer supports.
   - **More threads = faster mining** but uses more of your CPU. Your computer
     may feel slower while mining.
   - Start with half your available threads and see how it feels. You can
     always adjust later.
4. Optionally check **Background Mining**. This tells the miner to run at low
   priority so your normal computer use isn't affected. Recommended if you want
   to mine while working.
5. Click **Start Mining**.

### What you'll see while mining

- **Status**: "Mining" (green) or "Idle" (gray).
- **Hash Rate**: How many puzzles per second your CPU is attempting. Higher is
  better, but don't worry if the number seems small -- that's normal for CPU
  mining.
- **Algorithm**: RandomX -- Shekyl's mining algorithm, designed for regular
  CPUs.
- **Network Mining** section: Shows the current difficulty (how hard the
  puzzles are), the block reward, and the target block time (120 seconds).

### Will I actually find a block?

Honestly? On mainnet, solo mining with a single CPU is a bit like buying a
lottery ticket -- the network difficulty is shared across all miners, and a
single computer represents a tiny fraction. But every block you *do* find is
100% yours, and your mining contributes to the network's health and
decentralization. On testnet, blocks come much more easily because there are
fewer miners.

### The 60-block lock

When you mine a block, the reward is **locked for 60 blocks** (about 2 hours).
This is a safety rule -- it prevents issues if the network temporarily
reorganizes the chain. After 60 blocks, the reward becomes part of your
spendable balance.

### Background mining

With the "Background Mining" checkbox enabled, the miner runs at the lowest
CPU priority. You probably won't notice it's running. This is a great way to
contribute to the network while going about your day.

### Important requirement

Mining requires that your daemon is running in **unrestricted mode**. If you
started `shekyld` with the `--restricted-rpc` flag, mining controls won't
work. For normal home use, the default (unrestricted) mode is fine.

---

## Staking: Earning Yield While Strengthening Privacy

Staking in Shekyl works differently from most other cryptocurrencies. There's
no delegation, no validators, and no slashing. Here's how it works:

### The basics

1. You **lock** some of your SKL for a chosen period of time.
2. During the lock period, your coins sit in a shared **accrual pool** with
   everyone else's staked coins.
3. When the lock expires, you **claim** your original stake plus a share of
   the emission pool as your reward.

You never hand control of your coins to anyone. They're locked by the protocol
itself -- even you can't spend them until the lock period ends.

### Choosing a tier

There are three staking tiers:

| Tier | Lock Period | Yield Multiplier |
|------|-------------|------------------|
| Tier 0 | Short | 1.0x |
| Tier 1 | Medium | 1.5x |
| Tier 2 | Long | 2.0x |

The **yield multiplier** means Tier 2 earners get twice the reward share
compared to the same amount staked in Tier 0. The trade-off is that your
coins are locked for longer.

### The privacy benefit

This is what makes Shekyl's staking special. When you stake, your coins are
pooled together with everyone else's. When you claim your rewards, the SKL
comes from the shared pool. An outside observer can't easily tell which
specific stake belongs to which person.

The Staking page describes this as **"accrual pool commingling"** -- a fancy
way of saying your coins mix together with everyone else's, giving you
**plausible deniability**. Staking isn't just about earning yield; it's
also participating in the network's privacy.

### Estimated APY

The Staking page shows an estimated annual percentage yield for each tier.
This number changes based on:

- How much total SKL is staked across the network.
- The current emission rate (how many new coins are created).
- Your chosen tier's multiplier.

The APY is an *estimate*, not a guarantee. It fluctuates with network
conditions.

---

## Chain Health: What's Happening on the Network

Click **Chain Health** in the sidebar for a full dashboard of network
statistics. Think of this as the "vital signs" monitor for the Shekyl network.

Here's what each metric means:

- **Stake Ratio**: The percentage of circulating SKL that's currently staked.
  A healthy network has broad staking participation.

- **Release Tempo**: A dynamic multiplier that adjusts how many new coins are
  created per block. Higher network activity can push this up slightly.

- **Burn Rate**: A small percentage of each transaction fee is permanently
  destroyed ("burned"), reducing the total supply over time.

- **Emission Share**: How much of new coin creation goes to stakers vs. miners.

- **Supply Progress**: How far along we are in Shekyl's total coin emission
  schedule. Unlike fiat currency, the total supply of SKL is mathematically
  limited.

- **Network Stats**: Block height (how many blocks exist), hash rate (total
  mining power), last block reward, transaction pool size, and the daemon
  version.

The Dashboard shows a compact version of this panel. The full Chain Health page
gives you everything.

---

## Settings and Network Switching

The **Settings** page lets you configure your wallet.

### Network selection

Shekyl has three networks:

- **MainNet** -- The real network. Real money. This is what you use normally.
- **TestNet** -- A practice network with fake coins. Use this to learn without
  risking real money. You'll see an amber banner across the top of the wallet
  when you're on testnet, with a link to a faucet where you can get free
  test coins.
- **StageNet** -- A pre-release testing network used by developers.

Click the network buttons to switch. The wallet will automatically update the
daemon connection URL.

### Daemon connection

If your daemon is running on a different computer or a non-standard port, you
can change the URL here. Click **Save & Reconnect** after making changes.

### Post-Quantum Security

This section shows the current state of Shekyl's quantum protection. You'll
see the signature scheme (Hybrid: Ed25519 + ML-DSA-65), the transaction
version, and details about the V4 roadmap.

### Wallet management

You can change your wallet password or close the wallet from here.

---

## Understanding Post-Quantum Security

You might have noticed a small green shield badge in the wallet header showing
something like "Ed25519 + ML-DSA-65." Here's what that means, in plain
language.

### The problem

Most cryptocurrencies secure transactions using math based on "elliptic
curves" (Ed25519 is one such system). Today's computers can't break this math
in any reasonable time. But scientists believe that **quantum computers** --
a fundamentally different type of computer being developed in labs around
the world -- could eventually crack it.

Nobody knows exactly when quantum computers will be powerful enough to pose a
real threat. It might be 10 years, it might be 30. But cryptocurrency
addresses are public, and blockchains are permanent. If someone records your
public key today and cracks it in 2045, they could steal your funds.

### Shekyl's solution

Shekyl doesn't wait for the problem. Every time you spend SKL, the transaction
is signed with **two** different signatures:

1. **Ed25519** -- A battle-tested classical signature. Secure against all
   known attacks with today's computers.
2. **ML-DSA-65** -- A post-quantum signature standardized by NIST (the U.S.
   National Institute of Standards and Technology). Designed to resist both
   classical and quantum attacks.

Both signatures must be valid for a transaction to go through. This "hybrid"
approach means:

- If Ed25519 is somehow broken, ML-DSA-65 still protects you.
- If ML-DSA-65 turns out to have a flaw, Ed25519 still protects you.
- An attacker would need to break *both* simultaneously. That's like needing
  to pick two completely different kinds of lock on the same door, at the same
  time.

### What's coming in V4

A future update (transaction version V4) will extend quantum protection to
the privacy layer itself:

- **Lattice-based ring signatures** will make the sender-hiding mechanism
  quantum-resistant.
- **KEM stealth addresses** will make the one-time receive addresses
  quantum-resistant.

Right now, the privacy layer uses classical cryptography. It's secure today,
and V4 will future-proof it.

### You don't need to do anything

This all happens automatically. You don't need to enable anything, click
any buttons, or understand the math. The wallet handles it behind the scenes.
The green shield badge is just there to let you know you're protected.

Click the badge to open the Help Center for more details.

---

## Staying Safe

Cryptocurrency gives you full control of your money, which means full
responsibility too. Here are the essentials:

### Protect your seed words

We covered this earlier, but it bears repeating: your 25-word mnemonic seed
is the master key to everything. Store it offline, in a safe place, and never
share it. No legitimate person will ever ask for it.

### Use a strong wallet password

Your wallet file is encrypted with your password. Use something long and
unique -- a passphrase of four or five random words works well.

### Keep your software updated

Download wallet updates only from the
[official GitHub releases page](https://github.com/Shekyl-Foundation/shekyl-gui-wallet/releases).
Verify the download hash if you want to be extra cautious (instructions in
[INSTALLATION.md](INSTALLATION.md)).

### Be careful with addresses

Always double-check the recipient address before sending. Cryptocurrency
transactions are irreversible. If you send to the wrong address, there's no
way to get the coins back.

### Use testnet first

If you're new to crypto, spend some time on **TestNet** first. Go to
**Settings**, switch to TestNet, get some free test coins from the faucet, and
practice sending, receiving, mining, and staking. It all works exactly like
mainnet but with fake coins, so there's zero risk.

### Lock or close your wallet

If you step away from your computer, close the wallet or lock your screen.
An open wallet on an unlocked computer is an open wallet.

---

## Glossary

| Term | Meaning |
|------|---------|
| **Address** | Your public identifier for receiving SKL. Starts with `SKL1`. Safe to share -- nobody can steal funds by knowing your address. |
| **Atomic Unit** | The smallest unit of SKL. 1 SKL = 1,000,000,000 atomic units. The wallet handles conversion for you. |
| **Block** | A bundle of transactions added to the blockchain roughly every 2 minutes. |
| **Block Height** | The sequential number of a block, starting from 0. Higher number = more recent. |
| **Block Reward** | New SKL created and given to the miner who finds a valid block. |
| **Burn Rate** | The percentage of transaction fees permanently destroyed, reducing supply. |
| **Daemon** | The background program (`shekyld`) that connects to the Shekyl network. Your wallet talks to the daemon. |
| **Difficulty** | A measure of how hard mining puzzles are. Adjusts automatically to keep blocks arriving every ~2 minutes. |
| **Emission** | The schedule by which new SKL is created and distributed. The total supply is capped. |
| **Emission Era** | Named phases in the emission schedule (e.g., Foundation, Growth), each with different characteristics. |
| **Hash Rate** | The speed at which your CPU attempts mining puzzles. Measured in hashes per second (H/s). |
| **Hybrid Signature** | Two signatures on every transaction: one classical (Ed25519) and one quantum-resistant (ML-DSA-65). |
| **Mnemonic Seed** | The 25 words that can fully restore your wallet. Treat these like a master password you can never change. |
| **ML-DSA-65** | A quantum-resistant signature algorithm standardized by NIST. Part of Shekyl's hybrid protection. |
| **Privacy** | Shekyl hides who sends to whom using ring signatures and stealth addresses. This is automatic. |
| **RandomX** | Shekyl's mining algorithm. Runs efficiently on regular CPUs. |
| **Release Multiplier** | A dynamic factor that adjusts block emission based on network activity. |
| **Ring Signature** | A privacy technique that mixes your transaction with decoys so nobody can tell which input is real. |
| **Stake Ratio** | The percentage of circulating supply currently locked in staking. |
| **Staking** | Locking SKL for a period to earn yield. Your coins commingle with others in a shared pool for privacy. |
| **Stealth Address** | A one-time address generated for each transaction so only the sender and receiver know the destination. |
| **Transaction Fee** | A small amount of SKL paid to miners for including your transaction in a block. |

---

## Getting More Help

- **In-wallet Help Center**: Click **Help** in the sidebar for quick-reference
  guides on mining, staking, and post-quantum cryptography.

- **Installation issues**: See [INSTALLATION.md](INSTALLATION.md) for
  platform-specific troubleshooting.

- **Source code and development**: Visit the
  [GitHub repository](https://github.com/Shekyl-Foundation/shekyl-gui-wallet).

- **Daemon setup**: See the
  [shekyl-core documentation](https://github.com/Shekyl-Foundation/shekyl-core).

---

*This guide is for Shekyl Wallet v0.1.x-beta. Wallet creation, opening, and
import are functional when `shekyl-wallet-rpc` is installed and accessible.
Staking transactions require the wallet2 FFI bridge (not yet completed).
Mining and chain health features work when connected to a running `shekyld`
daemon.*
