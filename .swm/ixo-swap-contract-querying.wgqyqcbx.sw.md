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

This message consists of 1 mandatory field:

*   `address`<swm-token data-swm-token=":ixo-swap/src/msg.rs:112:5:5:`    Balance { address: String },`"/> -
<!-- NOTE-swimm-snippet: the lines below link your snippet to Swimm -->
### 📄 ixo-swap/src/msg.rs
```renderscript
112        Balance { address: String },
```

<br/>

### Response

```
pub struct BalanceResponse {
    pub balance: Uint128,
}
```

### Example

## Info

In order to get current state of contract we need to send `Info`<swm-token data-swm-token=":ixo-swap/src/msg.rs:114:1:1:`    Info {},`"/> message to a contract.

### Message

<br/>

This message does not require any fields.
<!-- NOTE-swimm-snippet: the lines below link your snippet to Swimm -->
### 📄 ixo-swap/src/msg.rs
```renderscript
114        Info {},
```

<br/>

### Response

<br/>


<!-- NOTE-swimm-snippet: the lines below link your snippet to Swimm -->
### 📄 ixo-swap/src/msg.rs
```renderscript
135    pub struct InfoResponse {
136        pub token1_reserve: Uint128,
137        pub token1_denom: Denom,
138        pub token2_reserve: Uint128,
139        pub token2_denom: Denom,
140        pub lp_token_supply: Uint128,
141        pub lp_token_address: String,
142    }
```

<br/>

### Example

## Token1155ForToken2Price

In order to get possible `Token2`<swm-token data-swm-token=":ixo-swap/src/msg.rs:31:1:1:`    Token2,`"/>amount based on `Token1155`<swm-token data-swm-token=":ixo-swap/src/msg.rs:30:1:1:`    Token1155,`"/> amount we need to send `Token1155ForToken2Price`<swm-token data-swm-token=":ixo-swap/src/msg.rs:116:1:1:`    Token1155ForToken2Price { token1155_amount: Uint128 },`"/> message to a contract.

### Message

<br/>

This message consists of 1 mandatory field:

*   `token1155_amount`<swm-token data-swm-token=":ixo-swap/src/msg.rs:116:5:5:`    Token1155ForToken2Price { token1155_amount: Uint128 },`"/> -
<!-- NOTE-swimm-snippet: the lines below link your snippet to Swimm -->
### 📄 ixo-swap/src/msg.rs
```renderscript
116        Token1155ForToken2Price { token1155_amount: Uint128 },
```

<br/>

### Response

<br/>


<!-- NOTE-swimm-snippet: the lines below link your snippet to Swimm -->
### 📄 ixo-swap/src/msg.rs
```renderscript
153    pub struct Token1155ForToken2PriceResponse {
154        pub token2_amount: Uint128,
155    }
```

<br/>

### Example

## Token2ForToken1155Price

In order to get possible `Token1155`<swm-token data-swm-token=":ixo-swap/src/msg.rs:30:1:1:`    Token1155,`"/> amount based on `Token2`<swm-token data-swm-token=":ixo-swap/src/msg.rs:31:1:1:`    Token2,`"/> amount we need to send `Token2ForToken1155Price`<swm-token data-swm-token=":ixo-swap/src/msg.rs:118:1:1:`    Token2ForToken1155Price { token2_amount: Uint128 },`"/> message to a contract.

### Message

<br/>

This message consists of 1 mandatory field:

*   `token2_amount`<swm-token data-swm-token=":ixo-swap/src/msg.rs:118:5:5:`    Token2ForToken1155Price { token2_amount: Uint128 },`"/> -
<!-- NOTE-swimm-snippet: the lines below link your snippet to Swimm -->
### 📄 ixo-swap/src/msg.rs
```renderscript
118        Token2ForToken1155Price { token2_amount: Uint128 },
```

<br/>

### Response

<br/>


<!-- NOTE-swimm-snippet: the lines below link your snippet to Swimm -->
### 📄 ixo-swap/src/msg.rs
```renderscript
158    pub struct Token2ForToken1155PriceResponse {
159        pub token1155_amount: Uint128,
160    }
```

<br/>

### Example

## Fee

In order to get fees we need to send `Fee`<swm-token data-swm-token=":ixo-swap/src/msg.rs:120:1:1:`    Fee {},`"/> message to a contract.

### Message

<br/>

This message does not require any fields.
<!-- NOTE-swimm-snippet: the lines below link your snippet to Swimm -->
### 📄 ixo-swap/src/msg.rs
```renderscript
120        Fee {},
```

<br/>

### Response

<br/>


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

## TokenSupplies

In order to get specific supplies of `Cw1155`<swm-token data-swm-token=":ixo-swap/src/msg.rs:25:1:1:`    Cw1155(Addr, String),`"/> tokens we need to send `TokenSupplies`<swm-token data-swm-token=":ixo-swap/src/msg.rs:122:1:1:`    TokenSupplies { tokens_id: Vec&lt;TokenId&gt; },`"/> message to a contract.

### Message

<br/>

This message consists of 1 mandatory field:

*   `tokens_id`<swm-token data-swm-token=":ixo-swap/src/msg.rs:122:5:5:`    TokenSupplies { tokens_id: Vec&lt;TokenId&gt; },`"/> -
<!-- NOTE-swimm-snippet: the lines below link your snippet to Swimm -->
### 📄 ixo-swap/src/msg.rs
```renderscript
122        TokenSupplies { tokens_id: Vec<TokenId> },
```

<br/>

### Response

<br/>


<!-- NOTE-swimm-snippet: the lines below link your snippet to Swimm -->
### 📄 ixo-swap/src/msg.rs
```renderscript
163    pub struct TokenSuppliesResponse {
164        pub supplies: Vec<Uint128>,
165    }
```

<br/>

### Example

<br/>

This file was generated by Swimm. [Click here to view it in the app](https://app.swimm.io/repos/Z2l0aHViJTNBJTNBaXhvLWNvbnRyYWN0cyUzQSUzQWl4b2ZvdW5kYXRpb24=/docs/wgqyqcbx).
