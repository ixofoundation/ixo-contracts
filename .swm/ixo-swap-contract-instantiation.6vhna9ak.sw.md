---
title: Ixo-swap contract instantiation
---

In this document will be discovered the instantiation of a contract.

## Message

<SwmSnippet path="/ixo-swap/src/msg.rs" line="13">

---

In order to instantiate the contract, we should send an appropriate message to the contract, in case of instantiation it should be <SwmToken path="/ixo-swap/src/msg.rs" pos="13:4:4" line-data="pub struct InstantiateMsg {">`InstantiateMsg`</SwmToken>.

```renderscript
pub struct InstantiateMsg {
    pub token1155_denom: Denom,
    pub token2_denom: Denom,
    pub lp_token_code_id: u64,
    pub max_slippage_percent: Decimal,
    pub protocol_fee_recipient: String,
    pub protocol_fee_percent: Decimal,
    pub lp_fee_percent: Decimal,
}
```

---

</SwmSnippet>

## Denom

<SwmSnippet path="/ixo-swap/src/msg.rs" line="24">

---

For instantiation we need to provide two tokens with one of supported <SwmToken path="/ixo-swap/src/msg.rs" pos="24:4:4" line-data="pub enum Denom {">`Denom`</SwmToken> for each. The first one is aways <SwmToken path="/ixo-swap/src/msg.rs" pos="27:1:1" line-data="    Cw1155(Addr, String),">`Cw1155`</SwmToken> token and the second one is either <SwmToken path="/ixo-swap/src/msg.rs" pos="26:1:1" line-data="    Cw20(Addr),">`Cw20`</SwmToken> or <SwmToken path="/ixo-swap/src/msg.rs" pos="25:1:1" line-data="    Native(String),">`Native`</SwmToken>.

- <SwmToken path="/ixo-swap/src/msg.rs" pos="25:1:1" line-data="    Native(String),">`Native`</SwmToken> needs a denom of token

- <SwmToken path="/ixo-swap/src/msg.rs" pos="26:1:1" line-data="    Cw20(Addr),">`Cw20`</SwmToken> needs an address of existing <SwmToken path="/ixo-swap/src/msg.rs" pos="26:1:1" line-data="    Cw20(Addr),">`Cw20`</SwmToken> contract

- <SwmToken path="/ixo-swap/src/msg.rs" pos="27:1:1" line-data="    Cw1155(Addr, String),">`Cw1155`</SwmToken> needs an address of existing <SwmToken path="/ixo-swap/src/msg.rs" pos="27:1:1" line-data="    Cw1155(Addr, String),">`Cw1155`</SwmToken> contract and supported denom of token

```renderscript
pub enum Denom {
    Native(String),
    Cw20(Addr),
    Cw1155(Addr, String),
}
```

---

</SwmSnippet>

## Fee

While instantiating, we need to specify 3 field for fee.

- <SwmToken path="/ixo-swap/src/msg.rs" pos="20:3:3" line-data="    pub lp_fee_percent: Decimal,">`lp_fee_percent`</SwmToken>- a contract fee percent for every swap. Basicly, the higher the percentage, the less a person who swap will receive

- <SwmToken path="/ixo-swap/src/msg.rs" pos="19:3:3" line-data="    pub protocol_fee_percent: Decimal,">`protocol_fee_percent`</SwmToken> - a fee that sends to <SwmToken path="/ixo-swap/src/msg.rs" pos="18:3:3" line-data="    pub protocol_fee_recipient: String,">`protocol_fee_recipient`</SwmToken> for every swap. This value is not taken from <SwmToken path="/ixo-swap/src/msg.rs" pos="20:3:3" line-data="    pub lp_fee_percent: Decimal,">`lp_fee_percent`</SwmToken> and is calculated separatly

- <SwmToken path="/ixo-swap/src/msg.rs" pos="18:3:3" line-data="    pub protocol_fee_recipient: String,">`protocol_fee_recipient`</SwmToken> - a person who receives <SwmToken path="/ixo-swap/src/msg.rs" pos="19:3:3" line-data="    pub protocol_fee_percent: Decimal,">`protocol_fee_percent`</SwmToken>for every swap

## Example

```json
{
  "token1_denom": {
    "cw1155": [
      "ixo1l6j9z82fvpn0mzkztmjz0zu78kj9nuh68vdd6czs8tq00ngltnxqxh9zwq",
      "CARBON"
    ]
  },
  "token2_denom": {
    "cw20": "ixo15hzg7eaxgs6ecn46gmu4juc9tau2w45l9cnf8n0797nmmtkdv7jsklrskg"
  },
  "lp_token_code_id": 25,
  "max_slippage_percent": "15.2",
  "protocol_fee_recipient": "ixo1rngxtm5sapzqdtw3k3e2e9zkjxzgpxd6vw9pye",
  "protocol_fee_percent": "0.1",
  "lp_fee_percent": "0.2"
}
```

<SwmMeta version="3.0.0" repo-id="Z2l0aHViJTNBJTNBaXhvLWNvbnRyYWN0cyUzQSUzQWl4b2ZvdW5kYXRpb24=" repo-name="ixo-contracts"><sup>Powered by [Swimm](https://app.swimm.io/)</sup></SwmMeta>
