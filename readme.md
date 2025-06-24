# MVP 1 – Core state setup

**Instruction `initialize`**

Deploys a singleton **`BurnerState`** PDA (seed = `b"state"`).

Stores:

* **authority** – admin wallet
* **is\_initialized** – flag to stop re‑init
* **created\_at** – Unix timestamp

Rent is paid by the caller.

Requires caller signature; nothing else can be done yet (no vaults, no token CPIs).

---

# MVP 2 – User vault lifecycle

Everything in **MVP 1**, plus:

**Instruction `create_vault`**

* Derives a per‑user **`VaultAccount`** PDA (seed = \[`b"vault"`, `user`]).
* Tracks **owner**, **bump**, and an optional **lamports\_collected** counter.

**Instruction `withdraw_vault`**

* Lets the vault owner pull all lamports above the rent‑exempt reserve into their wallet.
* Emits **`InvalidOwner`** if anyone else tries.
* Moves SOL with a manual lamport transfer while keeping the PDA alive.

### Security highlights

* Vault ownership enforced via PDA seeds and explicit owner check.
* Manual SOL movement keeps asset flow explicit; no arbitrary destinations.

### Still missing

* Actual token‑burn logic.
* **Address Lookup Table (ALT)** workflow.
* **Token‑2022** compatibility.

---

# MVP 3 – Token account validation

Everything in **MVP 2**, plus:

**Instruction `validate_token_account`**

* Derives **no new PDAs** – read‑only.
* Confirms the supplied SPL token account is:

  * **owned by the signer** (blocks probing strangers’ ATAs);
  * **a genuine SPL `TokenAccount`** (Anchor deserialization guard).
* Emits a log with **mint, balance, owner** and whether the account is empty (ready to close) or still holds tokens.

### Security highlights

* Explicit **owner check** ties the account to the signer.
* **Zero state mutation & no CPIs** – minimal attack surface.

### Still missing

* Actual **token‑burn / close** instruction that destroys tokens and reclaims rent.
* **Rent refund** pathway – credit lamports from closed accounts back to the user’s vault PDA.
* Batch workflows using **Address Lookup Tables (ALT)** for multi‑account burns.
* Compatibility layer for **SPL Token‑2022** and future extensions.
