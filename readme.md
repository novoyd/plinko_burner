# MVP 1 – Core state setup

Instruction: initialize

Deploys a singleton BurnerState PDA (seed = b"state").

Stores:  

authority – admin wallet. 

is_initialized – flag to stop re‑init.

created_at – Unix timestamp.

Rent is paid by the caller.

Requires caller signature; nothing else can be done yet (no vaults, no token CPIs).

# MVP 2 – User vault lifecycle

Everything in MVP 1, plus:

Instruction  create_vault

Derives a per‑user vault PDA (seed = [b"vault", user]).  

Tracks owner, bump, and an optional lamports_collected counter.

Instruction withdraw_vault

Lets the vault owner pull all lamports above the rent‑exempt reserve into their wallet.  

Emits InvalidOwner if anyone else tries.  

Moves SOL with a manual lamport transfer while keeping the PDA alive.  

Security highlights  

Vault ownership enforced via PDA seeds and explicit owner check.  

Manual SOL movement keeps asset flow explicit; no arbitrary destinations.  

Still missing  

Actual token‑burn logic.  

Address Lookup Table (ALT) workflow.  

Token‑2022 compatibility.  

