---
title: Ixo-swap contract querying
---

In this document will be discovered the querying of a contract.

# Messages

<SwmSnippet path="/ixo-swap/src/msg.rs" line="87">

---

In order to query the contract, we should send an appropriate message to the contract, in case of querying it should be <SwmToken path="/ixo-swap/src/msg.rs" pos="87:4:4" line-data="pub enum QueryMsg {">`QueryMsg`</SwmToken>:

```renderscript
pub enum QueryMsg {
```

---

</SwmSnippet>

## Balance

In order to get current balance of <SwmToken path="/ixo-swap/src/msg.rs" pos="25:1:1" line-data="    Cw20(Addr),">`Cw20`</SwmToken> for specific address we need to send <SwmToken path="/ixo-swap/src/msg.rs" pos="90:1:1" line-data="    Balance { address: String },">`Balance`</SwmToken> message to a contract.

### Message

<SwmSnippet path="/ixo-swap/src/msg.rs" line="90">

---

Message consists of 1 mandatory field:

- <SwmToken path="/ixo-swap/src/msg.rs" pos="90:5:5" line-data="    Balance { address: String },">`address`</SwmToken> - address of wallet

```renderscript
    Balance { address: String },
```

---

</SwmSnippet>

### Response

Response consists of 1 field:

- `balance` - amount of <SwmToken path="/ixo-swap/src/msg.rs" pos="25:1:1" line-data="    Cw20(Addr),">`Cw20`</SwmToken> token for provided address

```rust
pub struct BalanceResponse {
    pub balance: Uint128,
}
```

### Example

```json
{
  "balance": {
    "address": "ixo1n8yrmeatsk74dw0zs95ess9sgzptd6thgjgcj2"
  }
}
```

## Info

In order to get current state of contract we need to send <SwmToken path="/ixo-swap/src/msg.rs" pos="92:1:1" line-data="    Info {},">`Info`</SwmToken> message to a contract.

### Message

<SwmSnippet path="/ixo-swap/src/msg.rs" line="92">

---

Message does not require any fields.

```renderscript
    Info {},
```

---

</SwmSnippet>

### Response

<SwmSnippet path="/ixo-swap/src/msg.rs" line="108">

---

Response consists of 6 fields:

- <SwmToken path="/ixo-swap/src/msg.rs" pos="109:3:3" line-data="    pub token1155_reserve: Uint128,">`token1155_reserve`</SwmToken> - total amount of <SwmToken path="/ixo-swap/src/msg.rs" pos="31:1:1" line-data="    Token1155,">`Token1155`</SwmToken> reserve on contract

- <SwmToken path="/ixo-swap/src/msg.rs" pos="110:3:3" line-data="    pub token1155_denom: Denom,">`token1155_denom`</SwmToken> - <SwmToken path="/ixo-swap/src/msg.rs" pos="23:4:4" line-data="pub enum Denom {">`Denom`</SwmToken> of the <SwmToken path="/ixo-swap/src/msg.rs" pos="31:1:1" line-data="    Token1155,">`Token1155`</SwmToken>

- <SwmToken path="/ixo-swap/src/msg.rs" pos="111:3:3" line-data="    pub token2_reserve: Uint128,">`token2_reserve`</SwmToken> - amount of <SwmToken path="/ixo-swap/src/msg.rs" pos="32:1:1" line-data="    Token2,">`Token2`</SwmToken> reserve on contract

- <SwmToken path="/ixo-swap/src/msg.rs" pos="112:3:3" line-data="    pub token2_denom: Denom,">`token2_denom`</SwmToken> - <SwmToken path="/ixo-swap/src/msg.rs" pos="23:4:4" line-data="pub enum Denom {">`Denom`</SwmToken> of the <SwmToken path="/ixo-swap/src/msg.rs" pos="32:1:1" line-data="    Token2,">`Token2`</SwmToken>

- <SwmToken path="/ixo-swap/src/msg.rs" pos="113:3:3" line-data="    pub lp_token_supply: Uint128,">`lp_token_supply`</SwmToken> - total amount of <SwmToken path="/ixo-swap/src/msg.rs" pos="25:1:1" line-data="    Cw20(Addr),">`Cw20`</SwmToken> liquidity pool token

- <SwmToken path="/ixo-swap/src/msg.rs" pos="114:3:3" line-data="    pub lp_token_address: String,">`lp_token_address`</SwmToken> - address of <SwmToken path="/ixo-swap/src/msg.rs" pos="25:1:1" line-data="    Cw20(Addr),">`Cw20`</SwmToken> liquidity pool contract

```renderscript
pub struct InfoResponse {
    pub token1155_reserve: Uint128,
    pub token1155_denom: Denom,
    pub token2_reserve: Uint128,
    pub token2_denom: Denom,
    pub lp_token_supply: Uint128,
    pub lp_token_address: String,
}
```

---

</SwmSnippet>

### Example

```json
{
  "info": {}
}
```

## Token1155ForToken2Price

In order to get possible <SwmToken path="/ixo-swap/src/msg.rs" pos="32:1:1" line-data="    Token2,">`Token2`</SwmToken>amount based on <SwmToken path="/ixo-swap/src/msg.rs" pos="31:1:1" line-data="    Token1155,">`Token1155`</SwmToken> amount we need to send <SwmToken path="/ixo-swap/src/msg.rs" pos="94:1:1" line-data="    Token1155ForToken2Price { token1155_amount: TokenAmount },">`Token1155ForToken2Price`</SwmToken> message to a contract.

### Message

<SwmSnippet path="/ixo-swap/src/msg.rs" line="94">

---

Message consists of 1 mandatory field:

- <SwmToken path="/ixo-swap/src/msg.rs" pos="94:5:5" line-data="    Token1155ForToken2Price { token1155_amount: TokenAmount },">`token1155_amount`</SwmToken> - amount of <SwmToken path="/ixo-swap/src/msg.rs" pos="31:1:1" line-data="    Token1155,">`Token1155`</SwmToken>

```renderscript
    Token1155ForToken2Price { token1155_amount: TokenAmount },
```

---

</SwmSnippet>

### Response

<SwmSnippet path="/ixo-swap/src/msg.rs" line="125">

---

Response consists of 1 field:

- <SwmToken path="/ixo-swap/src/msg.rs" pos="126:3:3" line-data="    pub token2_amount: Uint128,">`token2_amount`</SwmToken> - possible amount of <SwmToken path="/ixo-swap/src/msg.rs" pos="32:1:1" line-data="    Token2,">`Token2`</SwmToken> based on <SwmToken path="/ixo-swap/src/msg.rs" pos="31:1:1" line-data="    Token1155,">`Token1155`</SwmToken> amount

```renderscript
pub struct Token1155ForToken2PriceResponse {
    pub token2_amount: Uint128,
}
```

---

</SwmSnippet>

### Example

```json
{
  "token1155_for_token2_price": {
    "token1155_amount": {
      "token1155": {
        "CARBON/1": "100",
        "CARBON/2": "100"
      }
    }
  }
}
```

## Token2ForToken1155Price

In order to get possible <SwmToken path="/ixo-swap/src/msg.rs" pos="31:1:1" line-data="    Token1155,">`Token1155`</SwmToken> amount based on <SwmToken path="/ixo-swap/src/msg.rs" pos="32:1:1" line-data="    Token2,">`Token2`</SwmToken> amount we need to send <SwmToken path="/ixo-swap/src/msg.rs" pos="96:1:1" line-data="    Token2ForToken1155Price { token2_amount: TokenAmount },">`Token2ForToken1155Price`</SwmToken> message to a contract.

### Message

<SwmSnippet path="/ixo-swap/src/msg.rs" line="96">

---

Message consists of 1 mandatory field:

- <SwmToken path="/ixo-swap/src/msg.rs" pos="96:5:5" line-data="    Token2ForToken1155Price { token2_amount: TokenAmount },">`token2_amount`</SwmToken> - amount of <SwmToken path="/ixo-swap/src/msg.rs" pos="32:1:1" line-data="    Token2,">`Token2`</SwmToken>

```renderscript
    Token2ForToken1155Price { token2_amount: TokenAmount },
```

---

</SwmSnippet>

### Response

<SwmSnippet path="/ixo-swap/src/msg.rs" line="130">

---

Response consists of 1 field:

- <SwmToken path="/ixo-swap/src/msg.rs" pos="126:3:3" line-data="    pub token2_amount: Uint128,">`token2_amount`</SwmToken> - possible amount of <SwmToken path="/ixo-swap/src/msg.rs" pos="31:1:1" line-data="    Token1155,">`Token1155`</SwmToken> based on <SwmToken path="/ixo-swap/src/msg.rs" pos="32:1:1" line-data="    Token2,">`Token2`</SwmToken> amount

```renderscript
pub struct Token2ForToken1155PriceResponse {
    pub token1155_amount: Uint128,
}
```

---

</SwmSnippet>

### Example

```json
{
  "token2_for_token1155_price": {
    "token2_amount": {
      "token2": "100"
    }
  }
}
```

## Fee

In order to get fees we need to send <SwmToken path="/ixo-swap/src/msg.rs" pos="98:1:1" line-data="    Fee {},">`Fee`</SwmToken> message to a contract.

### Message

<SwmSnippet path="/ixo-swap/src/msg.rs" line="98">

---

Message does not require any fields.

```renderscript
    Fee {},
```

---

</SwmSnippet>

### Response

<SwmSnippet path="/ixo-swap/src/msg.rs" line="118">

---

Response consists of 3 field:

- <SwmToken path="/ixo-swap/src/msg.rs" pos="119:3:3" line-data="    pub lp_fee_percent: Decimal,">`lp_fee_percent`</SwmToken> - a contract fee percent for every swap

- <SwmToken path="/ixo-swap/src/msg.rs" pos="120:3:3" line-data="    pub protocol_fee_percent: Decimal,">`protocol_fee_percent`</SwmToken> - a fee that sends to <SwmToken path="/ixo-swap/src/msg.rs" pos="121:3:3" line-data="    pub protocol_fee_recipient: String,">`protocol_fee_recipient`</SwmToken> for every swap

- <SwmToken path="/ixo-swap/src/msg.rs" pos="121:3:3" line-data="    pub protocol_fee_recipient: String,">`protocol_fee_recipient`</SwmToken> - a person who receives <SwmToken path="/ixo-swap/src/msg.rs" pos="120:3:3" line-data="    pub protocol_fee_percent: Decimal,">`protocol_fee_percent`</SwmToken> for every swap

```renderscript
pub struct FeeResponse {
    pub lp_fee_percent: Decimal,
    pub protocol_fee_percent: Decimal,
    pub protocol_fee_recipient: String,
}
```

---

</SwmSnippet>

### Example

```json
{
  "fee": {}
}
```

## TokenSupplies

In order to get specific supplies of <SwmToken path="/ixo-swap/src/msg.rs" pos="26:1:1" line-data="    Cw1155(Addr, String),">`Cw1155`</SwmToken> batches we need to send <SwmToken path="/ixo-swap/src/msg.rs" pos="100:1:1" line-data="    TokenSupplies { tokens_id: Vec&lt;TokenId&gt; },">`TokenSupplies`</SwmToken> message to a contract.

### Message

<SwmSnippet path="/ixo-swap/src/msg.rs" line="100">

---

Message consists of 1 mandatory field:

- <SwmToken path="/ixo-swap/src/msg.rs" pos="100:5:5" line-data="    TokenSupplies { tokens_id: Vec&lt;TokenId&gt; },">`tokens_id`</SwmToken> - ids of <SwmToken path="/ixo-swap/src/msg.rs" pos="26:1:1" line-data="    Cw1155(Addr, String),">`Cw1155`</SwmToken>batches

```renderscript
    TokenSupplies { tokens_id: Vec<TokenId> },
```

---

</SwmSnippet>

### Response

<SwmSnippet path="/ixo-swap/src/msg.rs" line="135">

---

Response consists of 1 field:

- <SwmToken path="/ixo-swap/src/msg.rs" pos="136:3:3" line-data="    pub supplies: Vec&lt;Uint128&gt;,">`supplies`</SwmToken> - total amounts of requested <SwmToken path="/ixo-swap/src/msg.rs" pos="26:1:1" line-data="    Cw1155(Addr, String),">`Cw1155`</SwmToken>batches

```renderscript
pub struct TokenSuppliesResponse {
    pub supplies: Vec<Uint128>,
}
```

---

</SwmSnippet>

### Example

```json
{
  "token_supplies": {
    "tokens_id": ["CARBON/1", "CARBON/2"]
  }
}
```

## FreezeStatus

In order to get freeze status of pools we need to send <SwmToken path="/ixo-swap/src/msg.rs" pos="102:1:1" line-data="    FreezeStatus {},">`FreezeStatus`</SwmToken> message to a contract.

### Message

<SwmSnippet path="/ixo-swap/src/msg.rs" line="102">

---

Message does not require any fields.

```
    FreezeStatus {},
```

---

</SwmSnippet>

### Response

<SwmSnippet path="/ixo-swap/src/msg.rs" line="140">

---

Response consists of 1 field:

- <SwmToken path="/ixo-swap/src/msg.rs" pos="141:3:3" line-data="    pub status: bool,">`status`</SwmToken> - current freeze status of pools

```
pub struct FreezeStatusResponse {
    pub status: bool,
}
```

---

</SwmSnippet>

### Example

```json
{
  "freeze_status": {}
}
```

## Ownership

In order to get ownership information we need to send <SwmToken path="/ixo-swap/src/msg.rs" pos="104:1:1" line-data="    Ownership {},">`Ownership`</SwmToken> message to a contract.

### Message

<SwmSnippet path="ixo-swap/src/msg.rs" line="104">

---

Message does not require any fields.

```
    Ownership {},
```

---

</SwmSnippet>

### Response

<SwmSnippet path="ixo-swap/src/msg.rs" line="145">

---

Response consists of 2 fields:

- <SwmToken path="/ixo-swap/src/msg.rs" pos="146:3:3" line-data="    pub owner: String,">`owner`</SwmToken> - current contract owner
- <SwmToken path="/ixo-swap/src/msg.rs" pos="147:3:3" line-data="    pub pending_owner: Option&lt;String&gt;,">`pending_owner`</SwmToken> - new contract owner, that needs to claim ownership

```
pub struct OwnershipResponse {
    pub owner: String,
    pub pending_owner: Option<String>,
}
```

---

</SwmSnippet>

### Example

```json
{
  "ownership": {}
}
```

<SwmMeta version="3.0.0" repo-id="Z2l0aHViJTNBJTNBaXhvLWNvbnRyYWN0cyUzQSUzQWl4b2ZvdW5kYXRpb24=" repo-name="ixo-contracts"><sup>Powered by [Swimm](https://app.swimm.io/)</sup></SwmMeta>
