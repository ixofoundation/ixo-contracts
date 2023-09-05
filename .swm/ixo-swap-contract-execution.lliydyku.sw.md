---
id: lliydyku
title: Ixo-swap contract execution
file_version: 1.1.3
app_version: 1.14.0
---

In this document will be discovered the execution of a contract.

# Messages

<br/>

In order to execute the contract, we should send an appropriate message to the contract, in case of execution it should be `ExecuteMsg`<swm-token data-swm-token=":ixo-swap/src/msg.rs:37:4:4:`pub enum ExecuteMsg {`"/>:
<!-- NOTE-swimm-snippet: the lines below link your snippet to Swimm -->
### 📄 ixo-swap/src/msg.rs
```renderscript
61     pub enum ExecuteMsg {
```

<br/>

## AddLiquidity

In order to add some liquidity to pool we need to send `AddLiquidity`<swm-token data-swm-token=":ixo-swap/src/msg.rs:38:1:1:`    AddLiquidity {`"/> message to a contract.

### Message

<br/>

Message consists of 3 mandatory fields:

*   `token1155_amounts`<swm-token data-swm-token=":ixo-swap/src/msg.rs:39:1:1:`        token1155_amounts: HashMap&lt;TokenId, Uint128&gt;,`"/> - `Cw1155`<swm-token data-swm-token=":ixo-swap/src/msg.rs:27:1:1:`    Cw1155(Addr, String),`"/> token amounts sender wants to add to the pool

*   `min_liquidity`<swm-token data-swm-token=":ixo-swap/src/msg.rs:40:1:1:`        min_liquidity: Uint128,`"/> - minimum expected amount of liquidity sender is ready to receive

*   `max_token2`<swm-token data-swm-token=":ixo-swap/src/msg.rs:41:1:1:`        max_token2: Uint128,`"/> - maximum expected amount of `Token2`<swm-token data-swm-token=":ixo-swap/src/msg.rs:33:1:1:`    Token2,`"/> sender is ready to spend

and 1 optional:

*   `expiration`<swm-token data-swm-token=":ixo-swap/src/msg.rs:69:1:1:`        expiration: Option&lt;Expiration&gt;,`"/> - block height or timestamp when message is no longer valid
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

```json
{
   "add_liquidity":{
      "token1155_amounts":{
         "CARBON/1":"50",
         "CARBON/2":"50"
      },
      "min_liquidity":"100",
      "max_token2":"100",
      "expiration":{
         "at_height":345543
      }
   }
}
```

## RemoveLiquidity

In order to remove liquidity from pool we need to send `RemoveLiquidity`<swm-token data-swm-token=":ixo-swap/src/msg.rs:44:1:1:`    RemoveLiquidity {`"/> message to a contract.

### Message

<br/>

Message consists of 3 mandatory fields:

*   `amount`<swm-token data-swm-token=":ixo-swap/src/msg.rs:45:1:1:`        amount: Uint128,`"/> - liquidity amount sender wants to remove from the pool

*   `min_token1155`<swm-token data-swm-token=":ixo-swap/src/msg.rs:46:1:1:`        min_token1155: TokenAmount,`"/> - minimum expected amount of `Token1155`<swm-token data-swm-token=":ixo-swap/src/msg.rs:32:1:1:`    Token1155,`"/> sedner is ready to receive. Could be either `Multiple`<swm-token data-swm-token=":ixo-swap/src/token_amount.rs:11:1:1:`    Multiple(HashMap&lt;TokenId, Uint128&gt;),`"/>, where sender specify batches he wants to receive or `Single`<swm-token data-swm-token=":ixo-swap/src/token_amount.rs:12:1:1:`    Single(Uint128),`"/>, where sender only specify total amount, leaving the selection of batches to the contract

*   `min_token2`<swm-token data-swm-token=":ixo-swap/src/msg.rs:47:1:1:`        min_token2: Uint128,`"/> - minimum expected amount of `Token2`<swm-token data-swm-token=":ixo-swap/src/msg.rs:33:1:1:`    Token2,`"/> sender is ready to receive

and 1 optional:

*   `expiration`<swm-token data-swm-token=":ixo-swap/src/msg.rs:69:1:1:`        expiration: Option&lt;Expiration&gt;,`"/> - block height or timestamp when message is no longer valid
<!-- NOTE-swimm-snippet: the lines below link your snippet to Swimm -->
### 📄 ixo-swap/src/msg.rs
```renderscript
68         RemoveLiquidity {
69             amount: Uint128,
70             min_token1155: TokenAmount,
71             min_token2: Uint128,
72             expiration: Option<Expiration>,
73         },
```

<br/>

### Example

```json
{
   "remove_liquidity":{
      "amount":"100",
      "min_token1155":{
         "multiple":{
            "CARBON/1":"50",
            "CARBON/2":"50"
         }
      },
      "min_token2":"100",
      "expiration":{
         "at_time":"2833211322185998723"
      }
   }
}
```

## Swap

In order to swap tokens on single contract we need to send `Swap`<swm-token data-swm-token=":ixo-swap/src/msg.rs:50:1:1:`    Swap {`"/> message to a contract.

### Message

<br/>

Message consists of 3 mandatory fields:

*   `input_token`<swm-token data-swm-token=":ixo-swap/src/msg.rs:65:1:1:`        input_token: TokenSelect,`"/> - selection of the token sender wants to swap

*   `input_amount`<swm-token data-swm-token=":ixo-swap/src/msg.rs:66:1:1:`        input_amount: TokenAmount,`"/> - amount of selected `input_token`<swm-token data-swm-token=":ixo-swap/src/msg.rs:65:1:1:`        input_token: TokenSelect,`"/>. In case `input_token`<swm-token data-swm-token=":ixo-swap/src/msg.rs:65:1:1:`        input_token: TokenSelect,`"/> is

    *   `Token1155`<swm-token data-swm-token=":ixo-swap/src/msg.rs:32:1:1:`    Token1155,`"/>, amount should only be `Multiple`<swm-token data-swm-token=":ixo-swap/src/token_amount.rs:11:1:1:`    Multiple(HashMap&lt;TokenId, Uint128&gt;),`"/>

    *   `Token2`<swm-token data-swm-token=":ixo-swap/src/msg.rs:33:1:1:`    Token2,`"/>, amount should only be `Single`<swm-token data-swm-token=":ixo-swap/src/token_amount.rs:12:1:1:`    Single(Uint128),`"/>

*   `min_output`<swm-token data-swm-token=":ixo-swap/src/msg.rs:53:1:1:`        min_output: TokenAmount,`"/> - minimum expected amount of another token from contract token pair, that means if `input_token`<swm-token data-swm-token=":ixo-swap/src/msg.rs:65:1:1:`        input_token: TokenSelect,`"/> is `Token1155`<swm-token data-swm-token=":ixo-swap/src/msg.rs:32:1:1:`    Token1155,`"/>, output will be `Token2`<swm-token data-swm-token=":ixo-swap/src/msg.rs:33:1:1:`    Token2,`"/> and vise versa. In case `min_output`<swm-token data-swm-token=":ixo-swap/src/msg.rs:53:1:1:`        min_output: TokenAmount,`"/> is

    *   `Token1155`<swm-token data-swm-token=":ixo-swap/src/msg.rs:32:1:1:`    Token1155,`"/>, amonut could be either `Multiple`<swm-token data-swm-token=":ixo-swap/src/token_amount.rs:11:1:1:`    Multiple(HashMap&lt;TokenId, Uint128&gt;),`"/>, where sender specify batches he wants to receive or `Single`<swm-token data-swm-token=":ixo-swap/src/token_amount.rs:12:1:1:`    Single(Uint128),`"/>, where sender only specify total amount, leaving the selection of batches to the contract

    *   `Token2`<swm-token data-swm-token=":ixo-swap/src/msg.rs:33:1:1:`    Token2,`"/>, amount should only be `Single`<swm-token data-swm-token=":ixo-swap/src/token_amount.rs:12:1:1:`    Single(Uint128),`"/>

and 1 optional:

*   `expiration`<swm-token data-swm-token=":ixo-swap/src/msg.rs:69:1:1:`        expiration: Option&lt;Expiration&gt;,`"/> - block height or timestamp when message is no longer valid
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

```json
{
   "swap":{
      "input_token":"token1155",
      "input_amount":{
         "multiple":{
            "CARBON/1":"50",
            "CARBON/2":"50"
         }
      },
      "min_output":{
         "single":"100"
      }
   }
}
```

## SwapAndSendTo

In order to swap tokens on single contract and send requested tokens to specific recipient we need to send `SwapAndSendTo`<swm-token data-swm-token=":ixo-swap/src/msg.rs:64:1:1:`    SwapAndSendTo {`"/> message to a contract.

### Message

<br/>

Message consists of 4 mandatory fields:

*   `input_token`<swm-token data-swm-token=":ixo-swap/src/msg.rs:65:1:1:`        input_token: TokenSelect,`"/>, `input_amount`<swm-token data-swm-token=":ixo-swap/src/msg.rs:66:1:1:`        input_amount: TokenAmount,`"/>, `min_token`<swm-token data-swm-token=":ixo-swap/src/msg.rs:68:1:1:`        min_token: TokenAmount,`"/> - see [Swap](https://app.swimm.io/workspaces/uD3gTrhLH5hUFWTf2PhX/repos/Z2l0aHViJTNBJTNBaXhvLWNvbnRyYWN0cyUzQSUzQWl4b2ZvdW5kYXRpb24=/docs/lliydyku#heading-Z1uCKnx)

*   `recipient`<swm-token data-swm-token=":ixo-swap/src/msg.rs:67:1:1:`        recipient: String,`"/> - address of the recipient, who will recieve the requested tokens

and 1 optional:

*   `expiration`<swm-token data-swm-token=":ixo-swap/src/msg.rs:69:1:1:`        expiration: Option&lt;Expiration&gt;,`"/> - block height or timestamp when message is no longer valid
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

In order to swap token from one contract for token from another contract we need to send `PassThroughSwap`<swm-token data-swm-token=":ixo-swap/src/msg.rs:57:1:1:`    PassThroughSwap {`"/> message to a contract.

For better understanding how it goes, lets see an example. The first and second contract have `Cw1155`<swm-token data-swm-token=":ixo-swap/src/msg.rs:27:1:1:`    Cw1155(Addr, String),`"/> and `Cw20`<swm-token data-swm-token=":ixo-swap/src/msg.rs:26:1:1:`    Cw20(Addr),`"/> token pairs. It's essential for both contract to have the same `Token2`<swm-token data-swm-token=":ixo-swap/src/msg.rs:33:1:1:`    Token2,`"/> whether it `Cw20`<swm-token data-swm-token=":ixo-swap/src/msg.rs:26:1:1:`    Cw20(Addr),`"/> with same address or `Native`<swm-token data-swm-token=":ixo-swap/src/msg.rs:25:1:1:`    Native(String),`"/> with same denom. So, sender wants to swap `Cw1155`<swm-token data-swm-token=":ixo-swap/src/msg.rs:27:1:1:`    Cw1155(Addr, String),`"/> token from first contract for `Cw1155`<swm-token data-swm-token=":ixo-swap/src/msg.rs:27:1:1:`    Cw1155(Addr, String),`"/> token from the second contract, thus we need to send `PassThroughSwap`<swm-token data-swm-token=":ixo-swap/src/msg.rs:57:1:1:`    PassThroughSwap {`"/> to the first contract. What contract does is firstly swap `Token1155`<swm-token data-swm-token=":ixo-swap/src/msg.rs:32:1:1:`    Token1155,`"/> for `Token2`<swm-token data-swm-token=":ixo-swap/src/msg.rs:33:1:1:`    Token2,`"/>, then send the `SwapAndSendTo`<swm-token data-swm-token=":ixo-swap/src/msg.rs:64:1:1:`    SwapAndSendTo {`"/> message to second contract with obtained `Token2`<swm-token data-swm-token=":ixo-swap/src/msg.rs:33:1:1:`    Token2,`"/> as `input_token`<swm-token data-swm-token=":ixo-swap/src/msg.rs:65:1:1:`        input_token: TokenSelect,`"/> after first swap, as both contract have same `Token2`<swm-token data-swm-token=":ixo-swap/src/msg.rs:33:1:1:`    Token2,`"/> we could easily swap it for `Cw1155`<swm-token data-swm-token=":ixo-swap/src/msg.rs:27:1:1:`    Cw1155(Addr, String),`"/> token from second contract , what we actually do in `SwapAndSendTo`<swm-token data-swm-token=":ixo-swap/src/msg.rs:64:1:1:`    SwapAndSendTo {`"/> after all requested `output_min_token`<swm-token data-swm-token=":ixo-swap/src/msg.rs:61:1:1:`        output_min_token: TokenAmount,`"/> will be send to sender of the `PassThroughSwap`<swm-token data-swm-token=":ixo-swap/src/msg.rs:57:1:1:`    PassThroughSwap {`"/> message.

### Message

<br/>

Message consists of 4 mandatory fields:

*   `input_token`<swm-token data-swm-token=":ixo-swap/src/msg.rs:65:1:1:`        input_token: TokenSelect,`"/> - see [Swap](https://app.swimm.io/workspaces/uD3gTrhLH5hUFWTf2PhX/repos/Z2l0aHViJTNBJTNBaXhvLWNvbnRyYWN0cyUzQSUzQWl4b2ZvdW5kYXRpb24=/docs/lliydyku#heading-Z1uCKnx)

*   `output_amm_address`<swm-token data-swm-token=":ixo-swap/src/msg.rs:58:1:1:`        output_amm_address: String,`"/> - address of the contract which has `output_min_token`<swm-token data-swm-token=":ixo-swap/src/msg.rs:61:1:1:`        output_min_token: TokenAmount,`"/>

*   `output_min_token`<swm-token data-swm-token=":ixo-swap/src/msg.rs:61:1:1:`        output_min_token: TokenAmount,`"/> - minimum expected amount of `Token1155`<swm-token data-swm-token=":ixo-swap/src/msg.rs:32:1:1:`    Token1155,`"/> from `output_amm_address`<swm-token data-swm-token=":ixo-swap/src/msg.rs:58:1:1:`        output_amm_address: String,`"/>.

and 1 optional:

*   `expiration`<swm-token data-swm-token=":ixo-swap/src/msg.rs:69:1:1:`        expiration: Option&lt;Expiration&gt;,`"/> - block height or timestamp when message is no longer valid
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

```json
{
   "pass_through_swap":{
      "output_amm_address":"ixo1l6j9z82fvpn0mzkztmjz0zu78kj9nuh68vdd6czs8tq00ngltnxqxh9zwq",
      "input_token":"token1155",
      "input_token_amount":{
         "multiple":{
            "CARBON/1":"50",
            "CARBON/2":"50"
         }
      },
      "output_min_token":{
         "multiple":{
            "WATER/1":"50",
            "WATER/2":"50"
         }
      }
   }
}
```

## UpdateConfig

In order to update contract configuration we need to send `UpdateConfig`<swm-token data-swm-token=":ixo-swap/src/msg.rs:71:1:1:`    UpdateConfig {`"/> message to a contract.

### Message

<br/>

Message consists of 3 mandatory fields:

*   `lp_fee_percent`<swm-token data-swm-token=":ixo-swap/src/msg.rs:73:1:1:`        lp_fee_percent: Decimal,`"/> - a contract fee percent for every swap

*   `protocol_fee_percent`<swm-token data-swm-token=":ixo-swap/src/msg.rs:74:1:1:`        protocol_fee_percent: Decimal,`"/> - a fee that sends to `protocol_fee_percent`<swm-token data-swm-token=":ixo-swap/src/msg.rs:74:1:1:`        protocol_fee_percent: Decimal,`"/> for every swap

*   `protocol_fee_recipient`<swm-token data-swm-token=":ixo-swap/src/msg.rs:75:1:1:`        protocol_fee_recipient: String,`"/> - a person who receives `protocol_fee_recipient`<swm-token data-swm-token=":ixo-swap/src/msg.rs:75:1:1:`        protocol_fee_recipient: String,`"/> for every swap

and 1 optional:

*   `owner`<swm-token data-swm-token=":ixo-swap/src/msg.rs:72:1:1:`        owner: Option&lt;String&gt;,`"/> - owner of a contract
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

In order to freeze or unfreeze deposits we need to send `FreezeDeposits`<swm-token data-swm-token=":ixo-swap/src/msg.rs:78:1:1:`    FreezeDeposits {`"/> message to a contract.

### Message

<br/>

Message consists of 1 mandatory field:

*   `freeze`<swm-token data-swm-token=":ixo-swap/src/msg.rs:79:1:1:`        freeze: bool,`"/> - freeze status
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
