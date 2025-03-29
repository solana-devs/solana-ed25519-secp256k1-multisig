# solana-secp256k1-multisig

1. **create_multisig** instruction initialize account with a list of eth wallets that able to propose tx or/and to approve some tx.
2. **create_transaction** instruction allow to propose tx for multisig. Any solana address can call this instruction but eth_wallet value as parameter of instruction should exists in list if owners of that multisig. Signed message should be vector of u8 [nonce, last_transaction_id] that remove possibility for attacker to reuse signature.
3. **approve** instruction allow to approve some transaction that was proposed on step 2. Any solana address can call this instruction but eth wallet value as parameter of instruction should exists in list if owners of that multisig.
   Signed message should be vector of u8 [nonce,last_transaction_id, 1] that remove possibility for attacker to reuse signature. value 1 at the end of message remove possibility for attacker to reuse signature that was used in create transaction step.
4. **execute_instruction** allow to execute the tx that was created on step 2 and approved on step 3. Anyone call this instructionin and will be excuted only if approvals list len > threshold.
5. All other instruction like **update_owners, update_threshold** are possible to call via **execute_instruction** because it require Auth context that will work only when tx approved by list of eth signers.

Test cases:

1. create_multisig
   - 🚫 InvalidThreshold if threshold value is 0 or bigger then owners list len.
   - 🚫 InvalidOwnersLen if owners list is empty.
   - :white_check_mark: PASS. owners list is not empty and threshold in range from 0 to owners len
2. create_transaction:
   - 🚫 Eth address not in multisig and wrong signature: Should Fail.
   - 🚫 Eth address in multisig but wrong signature: Should Fail.
   - 🚫 Eth address in multisig, correct signature but WRONG MSG: Should Fail! Not allow to reuse random signature for valid eth multisig owner.
   - :white_check_mark: valid eth address that exists in multisig owner list, correct signature and correct MSG(nonce + last_tx_id). Should PASS.
   - 🚫 Failed to reuse the same signature to create tx with dublicated tx id. Eth address in multisig address, correct signature but DUBLICATED MSG from past: Should Fail! Not allow to reuse random signature for valid eth multisig owner.
3. approve:
   - 🚫 Eth address not in multisig and wrong signature: Should Fail.
   - 🚫 Eth address in multisig but wrong signature: Should Fail.
   - 🚫 Eth address in multisig, correct signature but WRONG MSG: Should Fail! Not allow to reuse random signature for valid eth multisig owner.
   - 🚫 Eth address in multisig, correct signature BUT incorrect MSG from create_transaction workflow (nonce + last_tx_id). Should Fail.
   - 🚫 AlreadyExecuted. Eth address in multisig, correct signature and correct MSG(nonce + last_tx_id + 1). Should PASS.
   - :white_check_mark: Eth address in multisig, correct signature and correct MSG(nonce + last_tx_id + 1). Should PASS.
4. execute_instruction:
   - 🚫 execute tx with less approvals then threshold should Fail
   - 🚫 execute tx that already was executed should fail
   - :white_check_mark: execute tx that has approvals more then threshold and tx did not executed in past should Should PASS.
