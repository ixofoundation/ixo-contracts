---
title: Ixo-swap contract execution
---

In this document will be discovered the execution of a contract.

# Messages

<SwmSnippet path="/ixo-swap/src/msg.rs" line="37">

---

In order to execute the contract, we should send an appropriate message to the contract, in case of execution it should be <SwmToken path="/ixo-swap/src/msg.rs" pos="37:4:4" line-data="pub enum ExecuteMsg {">`ExecuteMsg`</SwmToken>:

```renderscript
pub enum ExecuteMsg {
```

---

</SwmSnippet>

## AddLiquidity

In order to add some liquidity to pool we need to send <SwmToken path="/ixo-swap/src/msg.rs" pos="38:1:1" line-data="    AddLiquidity {">`AddLiquidity`</SwmToken> message to a contract.

### Message

<SwmSnippet path="/ixo-swap/src/msg.rs" line="38">

---

Message consists of 3 mandatory fields:

- <SwmToken path="/ixo-swap/src/msg.rs" pos="39:1:1" line-data="        token1155_amounts: HashMap&lt;TokenId, Uint128&gt;,">`token1155_amounts`</SwmToken> - <SwmToken path="/ixo-swap/src/msg.rs" pos="27:1:1" line-data="    Cw1155(Addr, String),">`Cw1155`</SwmToken> token amounts sender wants to add to the pool

- <SwmToken path="/ixo-swap/src/msg.rs" pos="40:1:1" line-data="        min_liquidity: Uint128,">`min_liquidity`</SwmToken> - minimum expected amount of liquidity sender is ready to receive

- <SwmToken path="/ixo-swap/src/msg.rs" pos="41:1:1" line-data="        max_token2: Uint128,">`max_token2`</SwmToken> - maximum expected amount of <SwmToken path="/ixo-swap/src/msg.rs" pos="33:1:1" line-data="    Token2,">`Token2`</SwmToken> sender is ready to spend

and 1 optional:

- <SwmToken path="/ixo-swap/src/msg.rs" pos="69:1:1" line-data="        expiration: Option&lt;Expiration&gt;,">`expiration`</SwmToken> - block height or timestamp when message is no longer valid

```renderscript
    AddLiquidity {
        token1155_amounts: HashMap<TokenId, Uint128>,
        min_liquidity: Uint128,
        max_token2: Uint128,
        expiration: Option<Expiration>,
    },
```

---

</SwmSnippet>

### Example

```json
{
  "add_liquidity": {
    "token1155_amounts": {
      "CARBON/1": "50",
      "CARBON/2": "50"
    },
    "min_liquidity": "100",
    "max_token2": "100",
    "expiration": {
      "at_height": 345543
    }
  }
}
```

## RemoveLiquidity

In order to remove liquidity from pool we need to send <SwmToken path="/ixo-swap/src/msg.rs" pos="44:1:1" line-data="    RemoveLiquidity {">`RemoveLiquidity`</SwmToken> message to a contract.

### Message

<SwmSnippet path="/ixo-swap/src/msg.rs" line="44">

---

Message consists of 3 mandatory fields:

- <SwmToken path="/ixo-swap/src/msg.rs" pos="45:1:1" line-data="        amount: Uint128,">`amount`</SwmToken> - liquidity amount sender wants to remove from the pool

- <SwmToken path="/ixo-swap/src/msg.rs" pos="46:1:1" line-data="        min_token1155: TokenAmount,">`min_token1155`</SwmToken> - minimum expected amount of <SwmToken path="/ixo-swap/src/msg.rs" pos="32:1:1" line-data="    Token1155,">`Token1155`</SwmToken> sedner is ready to receive. Could be either <SwmToken path="/ixo-swap/src/token_amount.rs" pos="14:1:1" line-data="    Multiple(HashMap&lt;TokenId, Uint128&gt;),">`Multiple`</SwmToken>, where sender specify batches he wants to receive or <SwmToken path="/ixo-swap/src/token_amount.rs" pos="15:1:1" line-data="    Single(Uint128),">`Single`</SwmToken>, where sender only specify total amount, leaving the selection of batches to the contract

- <SwmToken path="/ixo-swap/src/msg.rs" pos="47:1:1" line-data="        min_token2: Uint128,">`min_token2`</SwmToken> - minimum expected amount of <SwmToken path="/ixo-swap/src/msg.rs" pos="33:1:1" line-data="    Token2,">`Token2`</SwmToken> sender is ready to receive

and 1 optional:

- <SwmToken path="/ixo-swap/src/msg.rs" pos="69:1:1" line-data="        expiration: Option&lt;Expiration&gt;,">`expiration`</SwmToken> - block height or timestamp when message is no longer valid

```renderscript
    RemoveLiquidity {
        amount: Uint128,
        min_token1155: TokenAmount,
        min_token2: Uint128,
        expiration: Option<Expiration>,
    },
```

---

</SwmSnippet>

### Example

```json
{
  "remove_liquidity": {
    "amount": "100",
    "min_token1155": {
      "multiple": {
        "CARBON/1": "50",
        "CARBON/2": "50"
      }
    },
    "min_token2": "100",
    "expiration": {
      "at_time": "2833211322185998723"
    }
  }
}
```

## Swap

In order to swap tokens on single contract we need to send <SwmToken path="/ixo-swap/src/msg.rs" pos="50:1:1" line-data="    Swap {">`Swap`</SwmToken> message to a contract.

### Message

<SwmSnippet path="/ixo-swap/src/msg.rs" line="50">

---

Message consists of 3 mandatory fields:

- <SwmToken path="/ixo-swap/src/msg.rs" pos="65:1:1" line-data="        input_token: TokenSelect,">`input_token`</SwmToken> - selection of the token sender wants to swap

- <SwmToken path="/ixo-swap/src/msg.rs" pos="66:1:1" line-data="        input_amount: TokenAmount,">`input_amount`</SwmToken> - amount of selected <SwmToken path="/ixo-swap/src/msg.rs" pos="65:1:1" line-data="        input_token: TokenSelect,">`input_token`</SwmToken>. In case <SwmToken path="/ixo-swap/src/msg.rs" pos="65:1:1" line-data="        input_token: TokenSelect,">`input_token`</SwmToken> is

  - <SwmToken path="/ixo-swap/src/msg.rs" pos="32:1:1" line-data="    Token1155,">`Token1155`</SwmToken>, amount should only be <SwmToken path="/ixo-swap/src/token_amount.rs" pos="14:1:1" line-data="    Multiple(HashMap&lt;TokenId, Uint128&gt;),">`Multiple`</SwmToken>

  - <SwmToken path="/ixo-swap/src/msg.rs" pos="33:1:1" line-data="    Token2,">`Token2`</SwmToken>, amount should only be <SwmToken path="/ixo-swap/src/token_amount.rs" pos="15:1:1" line-data="    Single(Uint128),">`Single`</SwmToken>

- <SwmToken path="/ixo-swap/src/msg.rs" pos="53:1:1" line-data="        min_output: TokenAmount,">`min_output`</SwmToken> - minimum expected amount of another token from contract token pair, that means if <SwmToken path="/ixo-swap/src/msg.rs" pos="65:1:1" line-data="        input_token: TokenSelect,">`input_token`</SwmToken> is <SwmToken path="/ixo-swap/src/msg.rs" pos="32:1:1" line-data="    Token1155,">`Token1155`</SwmToken>, output will be <SwmToken path="/ixo-swap/src/msg.rs" pos="33:1:1" line-data="    Token2,">`Token2`</SwmToken> and vise versa. In case <SwmToken path="/ixo-swap/src/msg.rs" pos="53:1:1" line-data="        min_output: TokenAmount,">`min_output`</SwmToken> is

  - <SwmToken path="/ixo-swap/src/msg.rs" pos="32:1:1" line-data="    Token1155,">`Token1155`</SwmToken>, amonut could be either <SwmToken path="/ixo-swap/src/token_amount.rs" pos="14:1:1" line-data="    Multiple(HashMap&lt;TokenId, Uint128&gt;),">`Multiple`</SwmToken>, where sender specify batches he wants to receive or <SwmToken path="/ixo-swap/src/token_amount.rs" pos="15:1:1" line-data="    Single(Uint128),">`Single`</SwmToken>, where sender only specify total amount, leaving the selection of batches to the contract

  - <SwmToken path="/ixo-swap/src/msg.rs" pos="33:1:1" line-data="    Token2,">`Token2`</SwmToken>, amount should only be <SwmToken path="/ixo-swap/src/token_amount.rs" pos="15:1:1" line-data="    Single(Uint128),">`Single`</SwmToken>

and 1 optional:

- <SwmToken path="/ixo-swap/src/msg.rs" pos="69:1:1" line-data="        expiration: Option&lt;Expiration&gt;,">`expiration`</SwmToken> - block height or timestamp when message is no longer valid

```renderscript
    Swap {
        input_token: TokenSelect,
        input_amount: TokenAmount,
        min_output: TokenAmount,
        expiration: Option<Expiration>,
    },
```

---

</SwmSnippet>

### Example

```json
{
  "swap": {
    "input_token": "token1155",
    "input_amount": {
      "multiple": {
        "CARBON/1": "50",
        "CARBON/2": "50"
      }
    },
    "min_output": {
      "single": "100"
    }
  }
}
```

## SwapAndSendTo

In order to swap tokens on single contract and send requested tokens to specific recipient we need to send <SwmToken path="/ixo-swap/src/msg.rs" pos="64:1:1" line-data="    SwapAndSendTo {">`SwapAndSendTo`</SwmToken> message to a contract.

### Message

<SwmSnippet path="/ixo-swap/src/msg.rs" line="64">

---

Message consists of 4 mandatory fields:

- <SwmToken path="/ixo-swap/src/msg.rs" pos="65:1:1" line-data="        input_token: TokenSelect,">`input_token`</SwmToken>, <SwmToken path="/ixo-swap/src/msg.rs" pos="66:1:1" line-data="        input_amount: TokenAmount,">`input_amount`</SwmToken>, <SwmToken path="/ixo-swap/src/msg.rs" pos="68:1:1" line-data="        min_token: TokenAmount,">`min_token`</SwmToken> - see [Swap](https://app.swimm.io/workspaces/uD3gTrhLH5hUFWTf2PhX/repos/Z2l0aHViJTNBJTNBaXhvLWNvbnRyYWN0cyUzQSUzQWl4b2ZvdW5kYXRpb24=/docs/lliydyku#heading-Z1uCKnx)

- <SwmToken path="/ixo-swap/src/msg.rs" pos="67:1:1" line-data="        recipient: String,">`recipient`</SwmToken> - address of the recipient, who will recieve the requested tokens

and 1 optional:

- <SwmToken path="/ixo-swap/src/msg.rs" pos="69:1:1" line-data="        expiration: Option&lt;Expiration&gt;,">`expiration`</SwmToken> - block height or timestamp when message is no longer valid

```renderscript
    SwapAndSendTo {
        input_token: TokenSelect,
        input_amount: TokenAmount,
        recipient: String,
        min_token: TokenAmount,
        expiration: Option<Expiration>,
    },
```

---

</SwmSnippet>

### Example

```
{
   "swap_and_send_to":{
      "input_token":"token2",
      "input_amount":{
         "single":"100"
      },
      "recipient":"ixo1n8yrmeatsk74dw0zs95ess9sgzptd6thgjgcj2",
      "min_token":{
         "single":"100"
      }
   }
}
```

## PassThroughSwap

In order to swap token from one contract for token from another contract we need to send <SwmToken path="/ixo-swap/src/msg.rs" pos="57:1:1" line-data="    PassThroughSwap {">`PassThroughSwap`</SwmToken> message to a contract.

For better understanding how it goes, lets see an example. The first and second contract have <SwmToken path="/ixo-swap/src/msg.rs" pos="27:1:1" line-data="    Cw1155(Addr, String),">`Cw1155`</SwmToken> and <SwmToken path="/ixo-swap/src/msg.rs" pos="26:1:1" line-data="    Cw20(Addr),">`Cw20`</SwmToken> token pairs. It's essential for both contract to have the same <SwmToken path="/ixo-swap/src/msg.rs" pos="33:1:1" line-data="    Token2,">`Token2`</SwmToken> whether it <SwmToken path="/ixo-swap/src/msg.rs" pos="26:1:1" line-data="    Cw20(Addr),">`Cw20`</SwmToken> with same address or <SwmToken path="/ixo-swap/src/msg.rs" pos="25:1:1" line-data="    Native(String),">`Native`</SwmToken> with same denom. So, sender wants to swap <SwmToken path="/ixo-swap/src/msg.rs" pos="27:1:1" line-data="    Cw1155(Addr, String),">`Cw1155`</SwmToken> token from first contract for <SwmToken path="/ixo-swap/src/msg.rs" pos="27:1:1" line-data="    Cw1155(Addr, String),">`Cw1155`</SwmToken> token from the second contract, thus we need to send <SwmToken path="/ixo-swap/src/msg.rs" pos="57:1:1" line-data="    PassThroughSwap {">`PassThroughSwap`</SwmToken> to the first contract. What contract does is firstly swap <SwmToken path="/ixo-swap/src/msg.rs" pos="32:1:1" line-data="    Token1155,">`Token1155`</SwmToken> for <SwmToken path="/ixo-swap/src/msg.rs" pos="33:1:1" line-data="    Token2,">`Token2`</SwmToken>, then send the <SwmToken path="/ixo-swap/src/msg.rs" pos="64:1:1" line-data="    SwapAndSendTo {">`SwapAndSendTo`</SwmToken> message to second contract with obtained <SwmToken path="/ixo-swap/src/msg.rs" pos="33:1:1" line-data="    Token2,">`Token2`</SwmToken> as <SwmToken path="/ixo-swap/src/msg.rs" pos="65:1:1" line-data="        input_token: TokenSelect,">`input_token`</SwmToken> after first swap, as both contract have same <SwmToken path="/ixo-swap/src/msg.rs" pos="33:1:1" line-data="    Token2,">`Token2`</SwmToken> we could easily swap it for <SwmToken path="/ixo-swap/src/msg.rs" pos="27:1:1" line-data="    Cw1155(Addr, String),">`Cw1155`</SwmToken> token from second contract , what we actually do in <SwmToken path="/ixo-swap/src/msg.rs" pos="64:1:1" line-data="    SwapAndSendTo {">`SwapAndSendTo`</SwmToken> after all requested <SwmToken path="/ixo-swap/src/msg.rs" pos="61:1:1" line-data="        output_min_token: TokenAmount,">`output_min_token`</SwmToken> will be send to sender of the <SwmToken path="/ixo-swap/src/msg.rs" pos="57:1:1" line-data="    PassThroughSwap {">`PassThroughSwap`</SwmToken> message.

### Message

<SwmSnippet path="/ixo-swap/src/msg.rs" line="57">

---

Message consists of 4 mandatory fields:

- <SwmToken path="/ixo-swap/src/msg.rs" pos="65:1:1" line-data="        input_token: TokenSelect,">`input_token`</SwmToken> - see [Swap](https://app.swimm.io/workspaces/uD3gTrhLH5hUFWTf2PhX/repos/Z2l0aHViJTNBJTNBaXhvLWNvbnRyYWN0cyUzQSUzQWl4b2ZvdW5kYXRpb24=/docs/lliydyku#heading-Z1uCKnx)

- <SwmToken path="/ixo-swap/src/msg.rs" pos="58:1:1" line-data="        output_amm_address: String,">`output_amm_address`</SwmToken> - address of the contract which has <SwmToken path="/ixo-swap/src/msg.rs" pos="61:1:1" line-data="        output_min_token: TokenAmount,">`output_min_token`</SwmToken>

- <SwmToken path="/ixo-swap/src/msg.rs" pos="61:1:1" line-data="        output_min_token: TokenAmount,">`output_min_token`</SwmToken> - minimum expected amount of <SwmToken path="/ixo-swap/src/msg.rs" pos="32:1:1" line-data="    Token1155,">`Token1155`</SwmToken> from <SwmToken path="/ixo-swap/src/msg.rs" pos="58:1:1" line-data="        output_amm_address: String,">`output_amm_address`</SwmToken>.

and 1 optional:

- <SwmToken path="/ixo-swap/src/msg.rs" pos="69:1:1" line-data="        expiration: Option&lt;Expiration&gt;,">`expiration`</SwmToken> - block height or timestamp when message is no longer valid

```renderscript
    PassThroughSwap {
        output_amm_address: String,
        input_token: TokenSelect,
        input_token_amount: TokenAmount,
        output_min_token: TokenAmount,
        expiration: Option<Expiration>,
    },
```

---

</SwmSnippet>

### Example

```json
{
  "pass_through_swap": {
    "output_amm_address": "ixo1l6j9z82fvpn0mzkztmjz0zu78kj9nuh68vdd6czs8tq00ngltnxqxh9zwq",
    "input_token": "token1155",
    "input_token_amount": {
      "multiple": {
        "CARBON/1": "50",
        "CARBON/2": "50"
      }
    },
    "output_min_token": {
      "multiple": {
        "WATER/1": "50",
        "WATER/2": "50"
      }
    }
  }
}
```

## UpdateFee

In order to update contract configuration we need to send <SwmToken path="/ixo-swap/src/msg.rs" pos="71:1:1" line-data="    UpdateFee {">`UpdateFee`</SwmToken> message to a contract.

### Message

<SwmSnippet path="/ixo-swap/src/msg.rs" line="71">

---

Message consists of 3 mandatory fields:

- <SwmToken path="/ixo-swap/src/msg.rs" pos="72:1:1" line-data="        lp_fee_percent: Decimal,">`lp_fee_percent`</SwmToken> - a contract fee percent for every swap

- <SwmToken path="/ixo-swap/src/msg.rs" pos="73:1:1" line-data="        protocol_fee_percent: Decimal,">`protocol_fee_percent`</SwmToken> - a fee that sends to <SwmToken path="/ixo-swap/src/msg.rs" pos="73:1:1" line-data="        protocol_fee_percent: Decimal,">`protocol_fee_percent`</SwmToken> for every swap

- <SwmToken path="/ixo-swap/src/msg.rs" pos="74:1:1" line-data="        protocol_fee_recipient: String,">`protocol_fee_recipient`</SwmToken> - a person who receives <SwmToken path="/ixo-swap/src/msg.rs" pos="74:1:1" line-data="        protocol_fee_recipient: String,">`protocol_fee_recipient`</SwmToken> for every swap

```
    UpdateFee {
        lp_fee_percent: Decimal,
        protocol_fee_percent: Decimal,
        protocol_fee_recipient: String,
    },
```

---

</SwmSnippet>

### Example

```json
{
  "update_config": {
    "protocol_fee_recipient": "ixo1rngxtm5sapzqdtw3k3e2e9zkjxzgpxd6vw9pye",
    "protocol_fee_percent": "0.1",
    "lp_fee_percent": "0.2"
  }
}
```

## UpdateSlippage

In order to update max slippage percent we need to send <SwmToken path="/ixo-swap/src/msg.rs" pos="76:1:1" line-data="    UpdateSlippage {">`UpdateSlippage`</SwmToken> message to a contract.

### Message

<SwmSnippet path="ixo-swap/src/msg.rs" line="76">

---

Message consists of 1 mandatory field:

- <SwmToken path="/ixo-swap/src/msg.rs" pos="77:1:1" line-data="        max_slippage_percent: Decimal,">`max_slippage_percent`</SwmToken> - maximum allower slippage percent

```
    UpdateSlippage {
        max_slippage_percent: Decimal,
    },
```

---

</SwmSnippet>

### Example

```json
{
  "update_slippage": {
    "max_slippage_percent": "5.5"
  }
}
```

## FreezeDeposits

In order to freeze or unfreeze deposits we need to send <SwmToken path="/ixo-swap/src/msg.rs" pos="84:1:1" line-data="    FreezeDeposits {">`FreezeDeposits`</SwmToken> message to a contract.

### Message

<SwmSnippet path="/ixo-swap/src/msg.rs" line="84">

---

Message consists of 1 mandatory field:

- <SwmToken path="/ixo-swap/src/msg.rs" pos="85:1:1" line-data="        freeze: bool,">`freeze`</SwmToken> - freeze status

```renderscript
    FreezeDeposits {
        freeze: bool,
    },
```

---

</SwmSnippet>

### Example

```json
{
  "freeze_deposits": {
    "freeze": false
  }
}
```

## TramsferOwnership

In order to transfer ownership of the contract we need to send <SwmToken path="/ixo-swap/src/msg.rs" pos="79:1:1" line-data="    TransferOwnership {">`TransferOwnership`</SwmToken> message to a contract.

### Message

<SwmSnippet path="/ixo-swap/src/msg.rs" line="79">

---

Message consists of 1 optional field:

- <SwmToken path="/ixo-swap/src/msg.rs" pos="80:1:1" line-data="        owner: Option&lt;String&gt;,">`owner`</SwmToken> - new owner of the contract

```
    TransferOwnership {
        owner: Option<String>,
    },
```

---

</SwmSnippet>

### Example

```json
{
  "transfer_ownership": {
    "owner": "ixo1rngxtm5sapzqdtw3k3e2e9zkjxzgpxd6vw9pye"
  }
}
```

## ClaimOwnership

In order to claim ownership for new contract onwer we need to send <SwmToken path="/ixo-swap/src/msg.rs" pos="82:1:1" line-data="    ClaimOwnership {},">`ClaimOwnership`</SwmToken> message to a contract.

### Message

<SwmSnippet path="/ixo-swap/src/msg.rs" line="82">

---

Message does not require any fields.

```
    ClaimOwnership {},
```

---

</SwmSnippet>

### Example

```json
{
  "claim_ownership": {}
}
```

<SwmMeta version="3.0.0" repo-id="Z2l0aHViJTNBJTNBaXhvLWNvbnRyYWN0cyUzQSUzQWl4b2ZvdW5kYXRpb24=" repo-name="ixo-contracts"><sup>Powered by [Swimm](https://app.swimm.io/)</sup></SwmMeta>
