This is the basic scenarios that must run.

1 
Alice wants to send message to Bob
Alice has no public IP, Bob has no public IP
Bob told Alice that he has his connection data at CentralMessageHub.  

Bob send to Alice his unique id and the address for CentralMessageHub (www.isageek.com:9979)

Alice asks CentralMessageHub to list their services. She gets:  

```
{
  "availability": "alwaysOn",
  "reachableAt": ["www.isageek.com.br:9979"],
  "buy" : [
    {
      "service": "store", 
      "credit":"own", 
      "description": "store some data for a period of time",
      "info": {
        "pricePerByte": 0.00000001, 
        "periodInDays": 10
      },
      "input": {
        "chunkId": "string",
        "data": "byteArray"
      },
      "output": {
        "success": "bool"
      }
    },
    {
      "service": "storeRecover", 
      "credit":"own", 
      "description": "recover stored data",
      "info": {
        "pricePerByte": 0.0000000000000001
      },
      "preCondition": "idChallenge",
      "input": {
        "chunkId": "string"
      },
      "output": {
        "data": "byteArray"
      }
    }
  ],
  "sell": [
    {
      "service": "store",
      "credit": "own", 
      "description": "store some data for a period of time",
      "info": {
        "pricePerByte": 0.00000001, 
        "periodInDays": 30
      },
      "input": {
        "chunkId": "string",
        "data": "byteArray"
      },
      "output": {
        "success": "bool"
      }
    },
    {
      "service": "storeRecover", 
      "credit":"own", 
      "description": "recover stored data",
      "info": {
        "pricePerByte": 0.0000000000000001
      },
      "preCondition": "idChallenge",
      "input": {
        "chunkId": "string"
      },
      "output": {
        "data": "byteArray"
      }
    },
    {
      "service": "storeEncryptedPackage", 
      "credit":"own", 
      "price": "0.01",
      "description": "store some data for a period of time that can be recovered by a different id",
      "info":{
        "pricePerByte": 0.00000001, 
        "periodInDays": 10
      },
      "preCondition": "idChallenge",
      "input": {
        "chunkId": "string"
      },
      "output": {
        "data": "byteArray"
      }
    },
    {
      "service": "searchEncryptedPackage", 
      "credit":"own", 
      "description": "Returns if there are pacckages for id in this server",
      "info":{
        "pricePerCall": 0.00000001
      },
      "preCondition": "idChallenge",
      "input": {
        "Id": "string"
      },
      "output": {
        "packageList": "PackageInfoArray"
      }
    },
    {
      "service": "getEncryptedPackage", 
      "credit":"own", 
      "description": "Returns a package from package info",
      "info":{
        "pricePerCall": 0.0001
      },
      "input": {
        "package": "packageInfo"
      },
      "output": {
        "packageContents": "byteArray"
      }
    },
    {
      "service": "getCurrentTemperature", 
      "credit":"own", 
      "description": "returns the current temperature",
      "info":{
        "pricePerCall": 1
      },
      "input": {
        "location": "string"
      },
      "output": {
        "weatherInCelsius": "int"
      }
    },
    {
      "service": "creditsBalance", 
      "credit":"own", 
      "description": "returns the current credits balance",
      "info":{
        "pricePerCall": 0.000000000001
      },
      "input": {
        "id": "string"
      },
      "output": {
        "credits": "string"
      }
    },
    {
      "service": "lookupID", 
      "credit":"own", 
      "description": "returns the public info for this ID",
      "info":{
        "pricePerCall": 0.0001
      },
      "input": {
        "id": "string"
      },
      "output": {
        "info": "profileInfo"
      }
    }
  ]
}


```

lookupID is the service that given an ID returns some profile info, including the public key for a user.  

But type of credits of service is own, which means a CentralMessageHub credits (if it was "other" it would be Alice credits)  

So, first step is to get some CentralMessageHub credits by looking at the buy entries.  

Alice sees she can get 1 credit by storing 100 megabytes for 10 days with a maximun of 1000 calls to recover the data.  
(She could pretend to store it and actually do nothing, but CentralMessageHub call challenges to check the data is there, and updates their internal trust score for Alice ID. If they caught her cheating, they can blacklist her ip, and even blacklist her in other nodes)  
Alice sells the storage, CentralMessageHub sends her 100 mega of encrypted data.  
CentralMessageHub then signs a message giving alice ID 1 credit and update their own ledger.  
Alice can now ask for Bob key.  
She calls lookupID for Bob id, she gets wathever info Bob wants public and also his pubkey.  
She calls storeEncryptedPackage with a message for Bob encrypted with his key and signed by her with her key.  
Message says "Hi! This is Alice!"

Bob (that already has CentralMessageHub) then runs searchEncryptedPackage for his ID.  
He finds the message from Alice. He calls getEncryptedPackage and get the message and Alice ID.  
He unencrypts it. Now he has her message and her ID.  
With her ID, bob gets her info and key from lookupID.  
The encrypted package actually does not need to be a message, but any data. Bob is also has credits for AlternativeMessageHub, so he sends the address for AlternativeMessageHub to Alice.  
Now they have a communication channel even if CentralMessageHub goes down.  
Alice stays connected with CentralMessageHub, this way she can send messages faster to Bob, but also CentralMessageHub can ask for their storage data. This also updates internal state on CentralMessageHub about trusting if Alice will be online when the data is needed, and the next time, the credits that CentralMessageHub is willing to pay will update accordingly.  

Alice is happy because she can now communicate with Bob, send him data or wathever.  
Bob is also Happy.  
CentralMessageHub is happy because they get storage. This is actually a redundant store they put in at least 5 nodes, ensuring any data almost 100% recovery chance. CentralMessageHub makes money by offering another unrelated service that uses the distributed storage.  

# Details, architecture, libraries, protocol, algorithms and technologies

This section turns the MVP story into concrete technical decisions. The goal is not to design the final network yet. The goal is to pick enough architecture, protocol, libraries, algorithms, and technologies to make the Alice-to-Bob scenario run with room to evolve.

## Details

The MVP should prove one complete asynchronous message flow:

1. Bob publishes an identity record at CentralMessageHub.
2. Alice asks CentralMessageHub for its service list.
3. Alice earns CentralMessageHub credits by accepting a storage challenge.
4. Alice spends CentralMessageHub credits to look up Bob, store an encrypted package for Bob, and later ask for delivery status.
5. Bob searches for packages addressed to his identity, downloads Alice's encrypted package, decrypts it, and verifies Alice's signature.
6. Bob can answer with alternate hub contact data so Alice and Bob are not permanently dependent on CentralMessageHub.

The MVP does not need to solve global decentralization yet. It should focus on a small set of reliable primitives: identity lookup, service discovery, credit accounting, encrypted package storage, encrypted package search, encrypted package retrieval, and storage challenge verification.

2

Alice puts a file public available for 1000 downloads using her credits. The file is spread through many nodes. 

3

A dashboard with public messages of selected people (social media) 

4

A personal web page but distributed 

## Architecture

The current code already has a useful separation:

- `network` owns TCP sockets and should only move framed bytes.
- `messageParsing` owns protocol interpretation and should evolve into protocol message handling.
- `ledger` owns local credit accounting.
- `offerings` owns the local service document as an opaque protocol document.
- `sharedStorage` should own storage challenge data and stored encrypted package bytes.
- `encryption` should own key generation, signing, verification, encryption, and decryption.
- `trust` should own trust events, trust score calculation, blacklisting decisions, and explanations of why an identity has its current trust state.

For the MVP, the socket layer should not know whether a request is `lookupID`, `storeEncryptedPackage`, or `creditsBalance`. It should read one complete frame, pass the payload to protocol handling, and send one complete response frame.

The service system should be described by contracts rather than hard-coded commands. This is important because TalkSphere services need to be flexible: one node may sell encrypted package storage, another may buy storage challenges, and another may sell an unrelated API such as temperature data. A client should be able to inspect the service list and understand what the service expects before calling it.

Each service offer should say:

- The operation name.
- Whether the node buys or sells the service.
- Which credit type is accepted or paid.
- The price.
- The input schema.
- The output schema.
- The maximum request size.
- The maximum response size.
- The expected payload encoding when bytes are involved.
- The error shape returned when the operation fails.

Example service entry:

```json
{
  "operation": "sell",
  "creditType": "own",
  "price": "0.01",
  "service": "storeEncryptedPackage",
  "input": {
    "recipientIdentityId": "string",
    "senderIdentityId": "string",
    "packageHash": "string",
    "expiresAt": "unixMilliseconds",
    "encryptedPayload": {
      "encoding": "base64",
      "maxBytes": 1048576
    },
    "senderSignature": "base64"
  },
  "output": {
    "packageId": "string",
    "storedUntil": "unixMilliseconds"
  },
  "errors": [
    "invalidSignature",
    "insufficientCredits",
    "payloadTooLarge",
    "storageUnavailable"
  ]
}
```

This does not mean every node must support arbitrary user-defined services on day one. The MVP should define a small required service set first:

- `listServices`
- `lookupIdentity`
- `creditsBalance`
- `acceptStorageChallenge`
- `answerStorageChallenge`
- `storeEncryptedPackage`
- `searchEncryptedPackage`
- `getEncryptedPackage`
- `deliveryReceipt`
- `trustStatus`

After those services work, flexible custom services can use the same service contract shape.

## Libraries

Suggested C libraries for the Linux MVP:

- JSON parsing: `jansson` for a practical MVP parser, or `cJSON` if we want the smallest learning curve. I would start with `jansson` because service descriptions will become nested documents quickly.
- Cryptography: `libsodium` for Ed25519 signatures, X25519 key exchange, XChaCha20-Poly1305 authenticated encryption, secure random bytes, and constant-time helpers.
- Hashing: BLAKE3 if we want speed and content addressing, or SHA-256 if we want fewer dependencies. The docs already mention BLAKE3 or SHA-256; for MVP, SHA-256 through `libsodium` is enough unless performance becomes important.
- Tests: keep the current shell/C test style for now. Add protocol parser tests for happy paths and malformed requests before adding network integration tests.

## Protocol

Decision: use binary framing for message boundaries, and use JSON for the framed protocol document.

The current socket code sends and receives plain text commands. That is fine for the first experiment, but it should not become the real MVP protocol because TCP does not preserve message boundaries. A receiver can get half a command, two commands together, or a large payload split across multiple reads.

The best MVP protocol is a hybrid:

- Use binary framing on the TCP stream.
- Use UTF-8 JSON as the framed protocol document.
- Put large encrypted bytes in a clearly declared field, either base64 inside JSON for small MVP payloads or a second binary frame for larger payloads later.

A minimal frame should be:

```text
4 bytes: payload length as unsigned big-endian integer
N bytes: UTF-8 JSON payload
```

The JSON payload should have an envelope:

```json
{
  "protocol": "talksphere",
  "version": 1,
  "messageId": "random-id",
  "kind": "request",
  "service": "lookupID",
  "paidBy": "alice-identity-id",
  "body": {
    "identityId": "bob-identity-id"
  }
}
```

And responses should mirror it:

```json
{
  "protocol": "talksphere",
  "version": 1,
  "messageId": "same-random-id",
  "kind": "response",
  "status": "ok",
  "body": {
    "identityId": "bob-identity-id",
    "publicSigningKey": "base64",
    "publicEncryptionKey": "base64",
    "reachableAt": [
      "www.isageek.com.br:9979"
    ]
  }
}
```

Error responses should use the same envelope:

```json
{
  "protocol": "talksphere",
  "version": 1,
  "messageId": "same-random-id",
  "kind": "response",
  "status": "error",
  "error": {
    "code": "insufficientCredits",
    "message": "CentralMessageHub credits are required for lookupID."
  }
}
```

JSON is a better MVP choice than XML here because it is smaller, easier to read, easier to map to service input/output descriptions, and common for API-like contracts. XML is useful when namespaces, mixed content, or document validation are the main problem. TalkSphere's MVP problem is structured service messages and encrypted bytes, so XML would add parser complexity without solving the main protocol risks.

Binary-only formats such as CBOR, MessagePack, or Protocol Buffers can come later. They are good when bandwidth and strict schemas matter more than debugging by inspection. For the MVP, readable JSON will make protocol mistakes easier to find.

The protocol should not use floating point numbers for prices. Prices should be encoded as decimal strings such as `"0.01"` in protocol documents, and converted into integer atomic units in code.

## Answers to open decisions

Should the socket payload be binary or text?

Use both, but at different layers. The socket transport should be binary because it needs length-prefixed frames. The protocol document inside the frame should be text JSON for the MVP.

Should service lists describe inputs and outputs?

Yes. Without input and output descriptions, the service list is only advertising names and prices. That is not flexible enough for TalkSphere because services are meant to be decentralized and node-defined. The service contract should describe what the caller sends, what the caller receives, sizes, encodings, and possible errors.

Is JSON the best option?

For the MVP, yes. JSON is the best practical option because it is readable, common, simple to debug with logs, and expressive enough for nested service contracts. The weakness is binary payloads, but the MVP can use base64 and later add binary payload frames.

Is XML better?

No for this MVP. XML has real strengths for document-centric formats, strict namespaces, and mature schema validation. Those are not the hard problems here. The hard problems are message boundaries, signatures, encrypted payloads, service contracts, credit semantics, and abuse resistance. XML would make parsing heavier without helping those core problems.

Should the protocol be a compact binary format from the start?

No. CBOR, MessagePack, or Protocol Buffers may be better later, but they would make the first implementation harder to inspect and debug. Once the JSON protocol stabilizes, a binary encoding can be added without changing the service concepts.

## Algorithms

- Identity IDs: random high-entropy IDs derived from public keys or generated independently and signed by the identity key.
- Signatures: sign service requests, stored packages, delivery receipts, and credit ledger events.
- Encryption: encrypt packages for the recipient identity key before they are sent to hubs.
- Package discovery: start with hub-local search where the caller proves control of the recipient identity by signing a hub challenge. Do not start with a full DHT until the hub MVP works.
- Credit accounting: start with a hub-local ledger signed by the hub. Later, add signed receipts so clients can prove credits and deliveries outside the hub.
- Storage verification: start with random byte-range challenges over encrypted data. Later, move toward stronger proof-of-storage or proof-of-retrievability schemes.
- Trust score: start as a local hub score based on successful challenge responses, failed responses, uptime, and delivery receipts.
- Routing: start with explicit `reachableAt` hub addresses. Later, model identities and hubs as a weighted graph and use Dijkstra or another shortest-path algorithm by cost, trust, and availability.

# Trust

Trust should be a separate domain folder because it is not the same responsibility as ledger, offerings, storage, or networking. The ledger says how many credits an identity has. Shared storage says whether bytes exist and whether challenges were answered. Offerings says what services this node is willing to buy or sell. Trust uses evidence from those domains to decide how much risk this node is willing to take with another identity.

For the MVP, `trust` should answer three questions:

1. Is this identity allowed to use or provide a service?
2. If allowed, should this identity get the normal price, a better price, or a worse price?
3. If the identity asks why its trust is low, what explanation can we give?

Trust should be local to the node that calculates it. CentralMessageHub can trust Alice highly while AlternativeMessageHub still knows nothing about her. Later, nodes may share signed trust events or recommendations, but the MVP should avoid treating trust as a global truth.

## Trust inputs

The first trust score should be calculated from simple events:

- Successful storage challenge responses.
- Failed storage challenge responses.
- Late storage challenge responses.
- Time connected while storage obligations are active.
- Completed agreements.
- Broken agreements.
- Valid signed delivery receipts.
- Invalid signatures or malformed requests.
- Repeated failed payments or attempts to use services without credits.
- Manual operator decisions such as temporary blocks or permanent blacklists.

Time online matters because storage is only useful when the holder can answer recovery calls. Alice may honestly keep the bytes, but if she is offline most of the time, her storage is less valuable to CentralMessageHub. That should not look exactly like cheating, but it should still lower the amount CentralMessageHub is willing to pay for Alice's storage.

## Trust outputs

Trust should produce decisions used by offerings:

- `allowed`: the identity can use the service at the calculated price.
- `limited`: the identity can use the service with smaller payloads, shorter periods, fewer concurrent agreements, or higher prices.
- `challengeRequired`: the identity must prove something before the service is accepted.
- `blocked`: the identity cannot use or provide this service right now.
- `blacklisted`: the identity is rejected because the node considers the risk too high.

Offerings should be personalized when the caller identity is known. An unknown or low-trust identity may see expensive storage offers, short storage periods, or no buying offer at all. A high-trust identity may see better prices, larger accepted payloads, longer periods, or more credit-earning opportunities.

New identities should start with low trust, not neutral trust. Creating a new identity is cheap, so starting new accounts at a comfortable trust level would make abuse cheap too. A node should make unknown identities pay higher prices, receive lower storage payments, accept smaller limits, or complete extra challenges until they build history. As the identity provides useful services and honors agreements, trust goes up and the node can pay more credits for that identity's storage, bandwidth, routing, or computation.

Example adjusted offering:

```json
{
  "operation": "buy",
  "creditType": "own",
  "basePrice": "1.00",
  "effectivePrice": "0.40",
  "service": "storage",
  "trustDecision": {
    "state": "limited",
    "score": 64,
    "reasonCodes": [
      "lowOnlineTime",
      "successfulChallenges"
    ]
  },
  "offerInfo": {
    "size": 100000,
    "period": 10,
    "maxRecoveryCalls": 1000
  }
}
```

## Trust explanations

Trust should be queryable through a `trustStatus` service. This is not only for debugging. It makes the economy understandable. If Alice receives bad storage offers, she should be able to ask CentralMessageHub why and learn that her online time is too low, her last challenge response was late, or she has too little history.

Example response:

```json
{
  "identityId": "alice-identity-id",
  "score": 64,
  "state": "limited",
  "summary": "Storage offers are limited because this identity has low online time during active storage agreements.",
  "reasons": [
    {
      "code": "lowOnlineTime",
      "message": "This identity was reachable for 42 percent of the last active storage period.",
      "target": "80 percent"
    },
    {
      "code": "successfulChallenges",
      "message": "This identity answered 19 of 20 storage challenges successfully."
    }
  ]
}
```

The explanation should avoid exposing private information about other identities. For example, it can say "invalid signatures were seen" or "delivery receipts were missing", but it should not reveal unrelated package metadata.

## Trust storage

The trust folder should store append-only trust events first, then calculate summaries from them. This is safer than only storing the final score because it lets the node explain the score and change the scoring formula later.

Suggested domain files:

```text
trust
  README.md
  trust_event.c
  trust_event.h
  trust_score.c
  trust_score.h
  trust_status.c
  trust_status.h
```

The trust module should not parse socket frames, mutate ledger balances, or inspect encrypted payload contents. It should receive primitive evidence from other domains, store the event, and calculate a decision when offerings or service handling ask for it.

## Trust scoring

The MVP can start with a simple weighted score from 0 to 100:

- Start unknown identities low, for example at 20.
- Add points for useful provided services, successful challenges, completed agreements, valid receipts, and long online time.
- Remove points for failed challenges, late responses, invalid signatures, broken agreements, and repeated unpaid requests.
- Decay old events so a bad start does not punish an identity forever.
- Keep hard blacklists separate from the numeric score because some decisions should not be averaged away.

This score should not pretend to be objective truth. It is a local risk estimate used by one node. The important part is that the score is explainable and that offerings can use it consistently. The economic rule should be simple: low trust means expensive access and low payments for provided services; higher trust means cheaper access, larger limits, and better credit payments.

# Package search authorization

Package search should not allow arbitrary lookup by recipient identity ID. CentralMessageHub may not read encrypted packages, but open search would still reveal who receives packages and when. The MVP should require the requester to prove control of the recipient identity before returning package IDs.

The flow should be:

1. Bob asks CentralMessageHub for a search challenge for Bob's identity ID.
2. CentralMessageHub returns a random nonce and short expiration time.
3. Bob signs the challenge with the private signing key for Bob's identity.
4. Bob sends `searchEncryptedPackage` with Bob's identity ID, the challenge ID, and the signature.
5. CentralMessageHub verifies the signature against Bob's public identity key.
6. CentralMessageHub returns only packages addressed to Bob's identity.

This does not hide all metadata from CentralMessageHub because the hub still stores and serves the packages. It does prevent Alice, Mallory, or any unrelated identity from asking CentralMessageHub whether Bob has pending packages.

## Trust and blacklisting

Blacklisting should mainly apply to identities, not IP addresses. IP addresses can be shared, temporary, or misleading. IP-based blocking can still be useful as a short-lived abuse-control tool, but identity-level trust is the main mechanism.

When a node blacklists an identity, it should record the reason as a trust event. That allows the user or operator to distinguish between "blocked because of repeated invalid signatures", "blocked because of unpaid service abuse", and "temporarily blocked because of suspicious traffic".

## Technologies

- Language: C for the current Linux runtime.
- Transport: TCP sockets for the MVP.
- Message format: length-prefixed JSON frames.
- Payload format: encrypted bytes, base64-encoded inside JSON for MVP-size packages.
- Crypto: Ed25519 for signatures, X25519 plus XChaCha20-Poly1305 for encryption through `libsodium`.
- Storage: local files first, with package metadata separated from package bytes.
- Trust: append-only local trust events plus calculated per-identity trust summaries.
- Build and tests: keep the existing Linux `Makefile` and test runner while the project is still small.

# Potential issues

- Credits are local promises, not global money. This is good for decentralization, but every request must clearly say which identity's credits are being used.
- A hub can lie about credits, delivery, storage, or identity data unless signed receipts and verification rules are part of the protocol.
- A client can accept a storage job and delete the bytes unless the hub performs challenges often enough and feeds those results into trust and future offerings.
- Open search by recipient identity ID would leak metadata. `searchEncryptedPackage` should require a signed challenge proving control of the recipient identity, so only Bob can search for Bob's packages.
- Base64 inside JSON is simple, but it increases payload size. It is acceptable for the MVP and should be replaced or extended with binary payload frames for larger data.
- Service schemas make services flexible, but too much flexibility can make clients hard to implement. The MVP should define a small required service set before allowing arbitrary services.
- Prices like `0.01` should not be represented as floating point numbers in code. Use decimal strings or integer atomic units.
- Public identity lookup creates a trust problem. Alice needs to know whether CentralMessageHub is authoritative for Bob, or whether Bob's identity record is signed by Bob.
- Blacklisting by IP is weak because many users share IPs or change networks. Identity-level trust should be the main abuse-control mechanism, with IP blocking only as a short-lived operational defense.
- If both Alice and Bob have no public IP, hubs become important availability anchors. The design should accept that hubs are useful without making a single hub required forever.
