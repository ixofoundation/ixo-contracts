---
id: lliydyku
title: Ixo-swap contract execution
file_version: 1.1.3
app_version: 1.14.0
---

In this document will be discovered the execution of a contract.

# Messages

<br/>

In order to execute the contract, we should send an appropriate message to the contract, in case of execution it should be `ExecuteMsg`<swm-token data-swm-token=":ixo-swap/src/msg.rs:61:4:4:`pub enum ExecuteMsg {`"/>:
<!-- NOTE-swimm-snippet: the lines below link your snippet to Swimm -->
### 📄 ixo-swap/src/msg.rs
```renderscript
61     pub enum ExecuteMsg {
```

<br/>

## AddLiquidity

In order to add some liquidity to pool we need to send `AddLiquidity`<swm-token data-swm-token=":ixo-swap/src/msg.rs:62:1:1:`    AddLiquidity {`"/> message to a contract.

### Message

<br/>

Message consists of 3 mandatory fields:

*   `token1155_amounts`<swm-token data-swm-token=":ixo-swap/src/msg.rs:63:1:1:`        token1155_amounts: HashMap&lt;TokenId, Uint128&gt;,`"/> -

*   `min_liquidity`<swm-token data-swm-token=":ixo-swap/src/msg.rs:64:1:1:`        min_liquidity: Uint128,`"/> -

*   `max_token2`<swm-token data-swm-token=":ixo-swap/src/msg.rs:65:1:1:`        max_token2: Uint128,`"/> -

and 1 optional:

*   `expiration`<swm-token data-swm-token=":ixo-swap/src/msg.rs:66:1:1:`        expiration: Option&lt;Expiration&gt;,`"/> - block height and timestamp when message is no longer valid
<!-- NOTE-swimm-snippet: the lines below link your snippet to Swimm -->
### 📄 ixo-swap/src/msg.rs
```renderscript
62         AddLiquidity {
63             token1155_amounts: HashMap<TokenId, Uint128>,
64             min_liquidity: Uint128,
65             max_token2: Uint128,
66             expiration: Option<Expiration>,
67         },
```

<br/>

### Example

## RemoveLiquidity

In order to remove liquidity from pool we need to send `RemoveLiquidity`<swm-token data-swm-token=":ixo-swap/src/msg.rs:68:1:1:`    RemoveLiquidity {`"/> message to a contract.

### Message

<br/>

Message consists of 3 mandatory fields:

*   `amount`<swm-token data-swm-token=":ixo-swap/src/msg.rs:69:1:1:`        amount: Uint128,`"/> -

*   `min_token1155`<swm-token data-swm-token=":ixo-swap/src/msg.rs:70:1:1:`        min_token1155: HashMap&lt;TokenId, Uint128&gt;,`"/> -

*   `min_token2`<swm-token data-swm-token=":ixo-swap/src/msg.rs:71:1:1:`        min_token2: Uint128,`"/> -

and 1 optional:

*   `expiration`<swm-token data-swm-token=":ixo-swap/src/msg.rs:66:1:1:`        expiration: Option&lt;Expiration&gt;,`"/> - block height and timestamp when message is no longer valid
<!-- NOTE-swimm-snippet: the lines below link your snippet to Swimm -->
### 📄 ixo-swap/src/msg.rs
```renderscript
68         RemoveLiquidity {
69             amount: Uint128,
70             min_token1155: HashMap<TokenId, Uint128>,
71             min_token2: Uint128,
72             expiration: Option<Expiration>,
73         },
```

<br/>

### Example

## Swap

In order to swap tokens on single contract we need to send `Swap`<swm-token data-swm-token=":ixo-swap/src/msg.rs:74:1:1:`    Swap {`"/> message to a contract.

### Message

<br/>

Message consists of 3 mandatory fields:

*   `input_token`<swm-token data-swm-token=":ixo-swap/src/msg.rs:75:1:1:`        input_token: TokenSelect,`"/> -

*   `input_amount`<swm-token data-swm-token=":ixo-swap/src/msg.rs:76:1:1:`        input_amount: TokenAmount,`"/> -

*   `min_output`<swm-token data-swm-token=":ixo-swap/src/msg.rs:77:1:1:`        min_output: TokenAmount,`"/> -

and 1 optional:

*   `expiration`<swm-token data-swm-token=":ixo-swap/src/msg.rs:66:1:1:`        expiration: Option&lt;Expiration&gt;,`"/> - block height and timestamp when message is no longer valid
<!-- NOTE-swimm-snippet: the lines below link your snippet to Swimm -->
### 📄 ixo-swap/src/msg.rs
```renderscript
74         Swap {
75             input_token: TokenSelect,
76             input_amount: TokenAmount,
77             min_output: TokenAmount,
78             expiration: Option<Expiration>,
79         },
```

<br/>

### Example

## SwapAndSendTo

In order to swap tokens on single contract and send output tokens to specific recipient we need to send `SwapAndSendTo`<swm-token data-swm-token=":ixo-swap/src/msg.rs:88:1:1:`    SwapAndSendTo {`"/> message to a contract.

### Message

<br/>

Message consists of 4 mandatory fields:

*   `input_token`<swm-token data-swm-token=":ixo-swap/src/msg.rs:75:1:1:`        input_token: TokenSelect,`"/> -

*   `input_amount`<swm-token data-swm-token=":ixo-swap/src/msg.rs:76:1:1:`        input_amount: TokenAmount,`"/> -

*   `recipient`<swm-token data-swm-token=":ixo-swap/src/msg.rs:91:1:1:`        recipient: String,`"/> -

*   `min_output`<swm-token data-swm-token=":ixo-swap/src/msg.rs:77:1:1:`        min_output: TokenAmount,`"/> -

and 1 optional:

*   `expiration`<swm-token data-swm-token=":ixo-swap/src/msg.rs:66:1:1:`        expiration: Option&lt;Expiration&gt;,`"/> - block height and timestamp when message is no longer valid
<!-- NOTE-swimm-snippet: the lines below link your snippet to Swimm -->
### 📄 ixo-swap/src/msg.rs
```renderscript
88         SwapAndSendTo {
89             input_token: TokenSelect,
90             input_amount: TokenAmount,
91             recipient: String,
92             min_token: TokenAmount,
93             expiration: Option<Expiration>,
94         },
```

<br/>

### Example

## PassThroughSwap

In order to swap token from one contract for token from another contract we need to send `PassThroughSwap`<swm-token data-swm-token=":ixo-swap/src/msg.rs:81:1:1:`    PassThroughSwap {`"/> message to a contract.

### Message

<br/>

Message consists of 4 mandatory fields:

*   `output_amm_address`<swm-token data-swm-token=":ixo-swap/src/msg.rs:82:1:1:`        output_amm_address: String,`"/>

*   `input_token`<swm-token data-swm-token=":ixo-swap/src/msg.rs:75:1:1:`        input_token: TokenSelect,`"/> -

*   `input_token_amount`<swm-token data-swm-token=":ixo-swap/src/msg.rs:84:1:1:`        input_token_amount: TokenAmount,`"/> -

*   `output_min_token`<swm-token data-swm-token=":ixo-swap/src/msg.rs:85:1:1:`        output_min_token: TokenAmount,`"/> -

and 1 optional:

*   `expiration`<swm-token data-swm-token=":ixo-swap/src/msg.rs:66:1:1:`        expiration: Option&lt;Expiration&gt;,`"/> - block height and timestamp when message is no longer valid
<!-- NOTE-swimm-snippet: the lines below link your snippet to Swimm -->
### 📄 ixo-swap/src/msg.rs
```renderscript
81         PassThroughSwap {
82             output_amm_address: String,
83             input_token: TokenSelect,
84             input_token_amount: TokenAmount,
85             output_min_token: TokenAmount,
86             expiration: Option<Expiration>,
87         },
```

<br/>

### Example

## UpdateConfig

In order to update contract configuration we need to send `UpdateConfig`<swm-token data-swm-token=":ixo-swap/src/msg.rs:95:1:1:`    UpdateConfig {`"/> message to a contract.

### Message

<br/>

Message consists of 3 mandatory fields:

*   `lp_fee_percent`<swm-token data-swm-token=":ixo-swap/src/msg.rs:97:1:1:`        lp_fee_percent: Decimal,`"/> -

*   `protocol_fee_percent`<swm-token data-swm-token=":ixo-swap/src/msg.rs:98:1:1:`        protocol_fee_percent: Decimal,`"/> -

*   `protocol_fee_recipient`<swm-token data-swm-token=":ixo-swap/src/msg.rs:99:1:1:`        protocol_fee_recipient: String,`"/> -

and 1 optional:

*   `owner`<swm-token data-swm-token=":ixo-swap/src/msg.rs:96:1:1:`        owner: Option&lt;String&gt;,`"/> - owner of a contract
<!-- NOTE-swimm-snippet: the lines below link your snippet to Swimm -->
### 📄 ixo-swap/src/msg.rs
```renderscript
95         UpdateConfig {
96             owner: Option<String>,
97             lp_fee_percent: Decimal,
98             protocol_fee_percent: Decimal,
99             protocol_fee_recipient: String,
100        },
```

<br/>

### Example

```json
{
   "update_config":{
      "owner":"ixo1n8yrmeatsk74dw0zs95ess9sgzptd6thgjgcj2",
      "protocol_fee_recipient":"ixo1rngxtm5sapzqdtw3k3e2e9zkjxzgpxd6vw9pye",
      "protocol_fee_percent":"0.1",
      "lp_fee_percent":"0.2"
   }
}
```

## FreezeDeposits

In order to freeze or unfreeze deposits we need to send `FreezeDeposits`<swm-token data-swm-token=":ixo-swap/src/msg.rs:102:1:1:`    FreezeDeposits {`"/> message to a contract.

### Message

<br/>

Message consists of 1 mandatory field:

*   `freeze`<swm-token data-swm-token=":ixo-swap/src/msg.rs:103:1:1:`        freeze: bool,`"/> - freeze status
<!-- NOTE-swimm-snippet: the lines below link your snippet to Swimm -->
### 📄 ixo-swap/src/msg.rs
```renderscript
102        FreezeDeposits {
103            freeze: bool,
104        },
```

<br/>

### Example

```json
{
   "freeze_deposits":{
      "freeze":false
   }
}
```

<br/>

This file was generated by Swimm. [Click here to view it in the app](https://app.swimm.io/repos/Z2l0aHViJTNBJTNBaXhvLWNvbnRyYWN0cyUzQSUzQWl4b2ZvdW5kYXRpb24=/docs/lliydyku).
