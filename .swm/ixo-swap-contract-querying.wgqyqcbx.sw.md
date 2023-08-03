---
id: wgqyqcbx
title: Ixo-swap contract querying
file_version: 1.1.3
app_version: 1.14.0
---

In this document will be discovered the querying of a contract.

# Messages

<br/>

In order to query the contract, we should send an appropriate message to the contract, in case of querying it should be `QueryMsg`<swm-token data-swm-token=":ixo-swap/src/msg.rs:109:4:4:`pub enum QueryMsg {`"/>:
<!-- NOTE-swimm-snippet: the lines below link your snippet to Swimm -->
### 📄 ixo-swap/src/msg.rs
```renderscript
109    pub enum QueryMsg {
```

<br/>

## Balance

In order to get current balance of `Cw20`<swm-token data-swm-token=":ixo-swap/src/msg.rs:24:1:1:`    Cw20(Addr),`"/> for specific address we need to send `Balance`<swm-token data-swm-token=":ixo-swap/src/msg.rs:112:1:1:`    Balance { address: String },`"/> message to a contract.

### Message

<br/>

Message consists of 1 mandatory field:

*   `address`<swm-token data-swm-token=":ixo-swap/src/msg.rs:112:5:5:`    Balance { address: String },`"/> - address of wallet
<!-- NOTE-swimm-snippet: the lines below link your snippet to Swimm -->
### 📄 ixo-swap/src/msg.rs
```renderscript
112        Balance { address: String },
```

<br/>

### Response

Response consists of 1 field:

*   `balance` - amount of `Cw20`<swm-token data-swm-token=":ixo-swap/src/msg.rs:24:1:1:`    Cw20(Addr),`"/> token for provided address

```
pub struct BalanceResponse {
    pub balance: Uint128,
}
```

### Example

```json
{
   "balance":{
      "address":"ixo1n8yrmeatsk74dw0zs95ess9sgzptd6thgjgcj2"
   }
}
```

## Info

In order to get current state of contract we need to send `Info`<swm-token data-swm-token=":ixo-swap/src/msg.rs:114:1:1:`    Info {},`"/> message to a contract.

### Message

<br/>

Message does not require any fields.
<!-- NOTE-swimm-snippet: the lines below link your snippet to Swimm -->
### 📄 ixo-swap/src/msg.rs
```renderscript
114        Info {},
```

<br/>

### Response

<br/>

Response consists of 6 fields:

*   `token1155_reserve`<swm-token data-swm-token=":ixo-swap/src/msg.rs:136:3:3:`    pub token1155_reserve: Uint128,`"/> - total amount of `Token1155`<swm-token data-swm-token=":ixo-swap/src/msg.rs:30:1:1:`    Token1155,`"/> reserve on contract

*   `token1155_denom`<swm-token data-swm-token=":ixo-swap/src/msg.rs:137:3:3:`    pub token1155_denom: Denom,`"/> - `Denom`<swm-token data-swm-token=":ixo-swap/src/msg.rs:22:4:4:`pub enum Denom {`"/> of the `Token1155`<swm-token data-swm-token=":ixo-swap/src/msg.rs:30:1:1:`    Token1155,`"/>

*   `token2_reserve`<swm-token data-swm-token=":ixo-swap/src/msg.rs:138:3:3:`    pub token2_reserve: Uint128,`"/> - amount of `Token2`<swm-token data-swm-token=":ixo-swap/src/msg.rs:31:1:1:`    Token2,`"/> reserve on contract

*   `token2_denom`<swm-token data-swm-token=":ixo-swap/src/msg.rs:139:3:3:`    pub token2_denom: Denom,`"/> - `Denom`<swm-token data-swm-token=":ixo-swap/src/msg.rs:22:4:4:`pub enum Denom {`"/> of the `Token2`<swm-token data-swm-token=":ixo-swap/src/msg.rs:31:1:1:`    Token2,`"/>

*   `lp_token_supply`<swm-token data-swm-token=":ixo-swap/src/msg.rs:140:3:3:`    pub lp_token_supply: Uint128,`"/> - total amount of `Cw20`<swm-token data-swm-token=":ixo-swap/src/msg.rs:24:1:1:`    Cw20(Addr),`"/> liquidity pool token

*   `lp_token_address`<swm-token data-swm-token=":ixo-swap/src/msg.rs:141:3:3:`    pub lp_token_address: String,`"/> - address of `Cw20`<swm-token data-swm-token=":ixo-swap/src/msg.rs:24:1:1:`    Cw20(Addr),`"/> liquidity pool contract
<!-- NOTE-swimm-snippet: the lines below link your snippet to Swimm -->
### 📄 ixo-swap/src/msg.rs
```renderscript
135    pub struct InfoResponse {
136        pub token1155_reserve: Uint128,
137        pub token1155_denom: Denom,
138        pub token2_reserve: Uint128,
139        pub token2_denom: Denom,
140        pub lp_token_supply: Uint128,
141        pub lp_token_address: String,
142    }
```

<br/>

### Example

```json
{
    "info":{
        
    }
}
```

## Token1155ForToken2Price

In order to get possible `Token2`<swm-token data-swm-token=":ixo-swap/src/msg.rs:31:1:1:`    Token2,`"/>amount based on `Token1155`<swm-token data-swm-token=":ixo-swap/src/msg.rs:30:1:1:`    Token1155,`"/> amount we need to send `Token1155ForToken2Price`<swm-token data-swm-token=":ixo-swap/src/msg.rs:116:1:1:`    Token1155ForToken2Price { token1155_amount: TokenAmount },`"/> message to a contract.

### Message

<br/>

Message consists of 1 mandatory field:

*   `token1155_amount`<swm-token data-swm-token=":ixo-swap/src/msg.rs:116:5:5:`    Token1155ForToken2Price { token1155_amount: TokenAmount },`"/> - amount of `Token1155`<swm-token data-swm-token=":ixo-swap/src/msg.rs:30:1:1:`    Token1155,`"/>
<!-- NOTE-swimm-snippet: the lines below link your snippet to Swimm -->
### 📄 ixo-swap/src/msg.rs
```renderscript
116        Token1155ForToken2Price { token1155_amount: TokenAmount },
```

<br/>

### Response

<br/>

Response consists of 1 field:

*   `token2_amount`<swm-token data-swm-token=":ixo-swap/src/msg.rs:154:3:3:`    pub token2_amount: Uint128,`"/> - possible amount of `Token2`<swm-token data-swm-token=":ixo-swap/src/msg.rs:31:1:1:`    Token2,`"/> based on `Token1155`<swm-token data-swm-token=":ixo-swap/src/msg.rs:30:1:1:`    Token1155,`"/> amount
<!-- NOTE-swimm-snippet: the lines below link your snippet to Swimm -->
### 📄 ixo-swap/src/msg.rs
```renderscript
153    pub struct Token1155ForToken2PriceResponse {
154        pub token2_amount: Uint128,
155    }
```

<br/>

### Example

```json
{
   "token1155_for_token2_price":{
      "token1155_amount":{
         "token1155":{
            "CARBON/1":"100",
            "CARBON/2":"100"
         }
      }
   }
}
```

## Token2ForToken1155Price

In order to get possible `Token1155`<swm-token data-swm-token=":ixo-swap/src/msg.rs:30:1:1:`    Token1155,`"/> amount based on `Token2`<swm-token data-swm-token=":ixo-swap/src/msg.rs:31:1:1:`    Token2,`"/> amount we need to send `Token2ForToken1155Price`<swm-token data-swm-token=":ixo-swap/src/msg.rs:118:1:1:`    Token2ForToken1155Price { token2_amount: TokenAmount },`"/> message to a contract.

### Message

<br/>

Message consists of 1 mandatory field:

*   `token2_amount`<swm-token data-swm-token=":ixo-swap/src/msg.rs:118:5:5:`    Token2ForToken1155Price { token2_amount: TokenAmount },`"/> - amount of `Token2`<swm-token data-swm-token=":ixo-swap/src/msg.rs:31:1:1:`    Token2,`"/>
<!-- NOTE-swimm-snippet: the lines below link your snippet to Swimm -->
### 📄 ixo-swap/src/msg.rs
```renderscript
118        Token2ForToken1155Price { token2_amount: TokenAmount },
```

<br/>

### Response

<br/>

Response consists of 1 field:

*   `token2_amount`<swm-token data-swm-token=":ixo-swap/src/msg.rs:154:3:3:`    pub token2_amount: Uint128,`"/> - possible amount of `Token1155`<swm-token data-swm-token=":ixo-swap/src/msg.rs:30:1:1:`    Token1155,`"/> based on `Token2`<swm-token data-swm-token=":ixo-swap/src/msg.rs:31:1:1:`    Token2,`"/> amount
<!-- NOTE-swimm-snippet: the lines below link your snippet to Swimm -->
### 📄 ixo-swap/src/msg.rs
```renderscript
158    pub struct Token2ForToken1155PriceResponse {
159        pub token1155_amount: Uint128,
160    }
```

<br/>

### Example

```
{
   "token2_for_token1155_price":{
      "token2_amount":{
         "token2":"100"
      }
   }
}
```

## Fee

In order to get fees we need to send `Fee`<swm-token data-swm-token=":ixo-swap/src/msg.rs:120:1:1:`    Fee {},`"/> message to a contract.

### Message

<br/>

Message does not require any fields.
<!-- NOTE-swimm-snippet: the lines below link your snippet to Swimm -->
### 📄 ixo-swap/src/msg.rs
```renderscript
120        Fee {},
```

<br/>

### Response

<br/>

Response consists of 4 field:

*   `owner`<swm-token data-swm-token=":ixo-swap/src/msg.rs:146:3:3:`    pub owner: Option&lt;String&gt;,`"/> - administrator of a contract, who can to manipulate contract

*   `lp_fee_percent`<swm-token data-swm-token=":ixo-swap/src/msg.rs:147:3:3:`    pub lp_fee_percent: Decimal,`"/> - a contract fee percent for every swap

*   `protocol_fee_percent`<swm-token data-swm-token=":ixo-swap/src/msg.rs:148:3:3:`    pub protocol_fee_percent: Decimal,`"/> - a fee that sends to `protocol_fee_recipient`<swm-token data-swm-token=":ixo-swap/src/msg.rs:149:3:3:`    pub protocol_fee_recipient: String,`"/> for every swap

*   `protocol_fee_recipient`<swm-token data-swm-token=":ixo-swap/src/msg.rs:149:3:3:`    pub protocol_fee_recipient: String,`"/> - a person who receives `protocol_fee_percent`<swm-token data-swm-token=":ixo-swap/src/msg.rs:148:3:3:`    pub protocol_fee_percent: Decimal,`"/> for every swap
<!-- NOTE-swimm-snippet: the lines below link your snippet to Swimm -->
### 📄 ixo-swap/src/msg.rs
```renderscript
145    pub struct FeeResponse {
146        pub owner: Option<String>,
147        pub lp_fee_percent: Decimal,
148        pub protocol_fee_percent: Decimal,
149        pub protocol_fee_recipient: String,
150    }
```

<br/>

### Example

```
{
    "fee":{
        
    }
}
```

## TokenSupplies

In order to get specific supplies of `Cw1155`<swm-token data-swm-token=":ixo-swap/src/msg.rs:25:1:1:`    Cw1155(Addr, String),`"/> batches we need to send `TokenSupplies`<swm-token data-swm-token=":ixo-swap/src/msg.rs:122:1:1:`    TokenSupplies { tokens_id: Vec&lt;TokenId&gt; },`"/> message to a contract.

### Message

<br/>

Message consists of 1 mandatory field:

*   `tokens_id`<swm-token data-swm-token=":ixo-swap/src/msg.rs:122:5:5:`    TokenSupplies { tokens_id: Vec&lt;TokenId&gt; },`"/> - ids of `Cw1155`<swm-token data-swm-token=":ixo-swap/src/msg.rs:25:1:1:`    Cw1155(Addr, String),`"/>batches
<!-- NOTE-swimm-snippet: the lines below link your snippet to Swimm -->
### 📄 ixo-swap/src/msg.rs
```renderscript
122        TokenSupplies { tokens_id: Vec<TokenId> },
```

<br/>

### Response

<br/>

Response consists of 1 field:

*   `supplies`<swm-token data-swm-token=":ixo-swap/src/msg.rs:164:3:3:`    pub supplies: Vec&lt;Uint128&gt;,`"/> - total amounts of requested `Cw1155`<swm-token data-swm-token=":ixo-swap/src/msg.rs:25:1:1:`    Cw1155(Addr, String),`"/>batches
<!-- NOTE-swimm-snippet: the lines below link your snippet to Swimm -->
### 📄 ixo-swap/src/msg.rs
```renderscript
163    pub struct TokenSuppliesResponse {
164        pub supplies: Vec<Uint128>,
165    }
```

<br/>

### Example

```
{
   "token_supplies":{
      "tokens_id":[
         "CARBON/1",
         "CARBON/2"
      ]
   }
}
```

<br/>

This file was generated by Swimm. [Click here to view it in the app](https://app.swimm.io/repos/Z2l0aHViJTNBJTNBaXhvLWNvbnRyYWN0cyUzQSUzQWl4b2ZvdW5kYXRpb24=/docs/wgqyqcbx).
