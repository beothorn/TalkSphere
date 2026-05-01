# Nothing to see here yet, this is just me playing with an idea


# TalkSphere

# TalkSphere

A trust-based, peer-to-peer network where communication, storage, and services are earned, not free.

---

## Our Current Situation

The internet promised an age of unlimited access to knowledge. Instead, we built a system optimized for attention, not relevance.

Today’s digital world has several structural problems:

- **Anyone can reach anyone at near-zero cost**  
  Result, spam, scams, and mass unsolicited communication

- **Centralized platforms control visibility**  
  A few companies decide what content is seen

- **Content is funded by ads**  
  Incentives favor engagement over relevance

- **AI consumes content without returning value**  
  Creators lose incentives as their work is extracted and reused

- **Hardware is extremely cheap and abundant**  
  Storage, bandwidth, and compute are inexpensive enough that companies can profit at scale even with low-value models like ads

- **Inequality in resource utilization**  
  Most users have unused storage and computation capacity, yet these resources are not shared  
  Making resources reachable and usable is difficult, creating an imbalance where centralized providers extract most of the value

---

## The Core Problem

### 1. Reachability is too cheap

In the real world:
- You talk to people close to you
- Reaching strangers requires effort
- Trust filters interactions naturally

Online:
- Anyone can contact you instantly
- No cost means no filter
- Bad actors scale infinitely

---

### 2. We are overwhelmed with irrelevant information

The modern internet does not optimize for relevance, it optimizes for engagement.

This leads to:
- Endless low-value content
- Repetition of the same ideas
- Artificial amplification of noise
- Content designed to provoke, not inform

Human attention is limited.

Every topic requires time to understand, evaluate, and contextualize.

But:
- We cannot evaluate everything
- We cannot verify everything
- We cannot filter everything ourselves

So we:
- Skim instead of analyze
- React instead of reflect
- Consume what is pushed, not what is relevant

---

### 3. Time is misallocated

The real scarcity is time.

We constantly spend time on:
- Content that is not relevant
- Content designed to capture attention
- Content selected by algorithms instead of trust

This creates a system where:
- Important information competes with noise
- Irrelevant content dominates attention
- Users lose control over what they consume

---

### 4. Natural social filtering is gone

Before:
- Information passed through trusted people
- Messages were filtered and contextualized
- Relevance was shaped socially

Now:
- Raw information reaches everyone
- No built-in trust layer
- No cost to distribute irrelevant or harmful content

---

### 5. Incentives are broken

Current model:
- Platforms profit from engagement
- Creators depend on visibility

Result:
- Irrelevance scales better than relevance
- Volume beats quality
- Noise outcompetes meaningful content

---

## How It Used to Work

In real-life social networks:

- Communication is local
- Trust is built over time
- Reachability is limited
- Relaying information has cost
- Information spreads through trusted paths

This creates:
- Natural resistance to spam
- Social filtering of relevance
- High cost for distributing irrelevant information

---

## The Idea Behind TalkSphere

What if the internet worked more like real social interaction?

TalkSphere introduces a network where access depends on trust, and communication has a cost.

Relevance emerges from trust, not algorithms.

In practice, a tool other applications comunicate with to find peers, store distributed data and get meta-data such as trust level.  
It can be used to build social networks and other simpler applications.  

---

## What is TalkSphere

TalkSphere is a peer-to-peer system for:

- Communication
- Data storage
- Service access

Where access is determined by:

- Trust relationships
- Resource contribution
- Local policies
- User-defined credits

---

## Core Ideas

### 1. Resource sharing based on tit-for-tat

Users contribute resources such as:
- Storage
- Computation
- Bandwidth

In return, they earn the ability to use resources from others.

Value is created by contribution, not attention.

---

### 2. Reachability based on trust

Each user decides:
- Who can contact them
- At what cost
- Under what conditions

Trust defines:
- Access
- Cost
- Relevance

Close relationships are cheap to reach, distant ones are expensive.

---

### 3. Hardware independence through redundancy

Data is not tied to a single device.

Instead:
- Data is replicated across multiple peers
- Availability emerges from redundancy
- As long as you contribute resources, your data remains accessible

This removes dependence on:
- Central servers
- Fixed infrastructure
- Always-online personal devices

---

## Credits System

### What are credits

A credit is a user-defined unit of value:

> "You can use my resources in exchange for this."

Examples:
- AliceCredits
- BobCredits
- ServiceCredits

There is no global currency.

---

### Earning credits

You earn credits by providing value:
- Storage
- Computation
- Message routing
- APIs
- Caching

---

### Spending credits

You spend credits to:
- Send messages
- Store data
- Access services

---

### Trust-based conversion

Credits only work between trusted peers.

- Strong trust, cheap conversion  
- Weak trust, expensive conversion  
- No trust, no conversion  

---

### Multi-hop conversion

Credits can flow through the network:

Alice → Bob → Carol → Dave


Each step:
- Converts credits
- Applies fees
- Depends on trust

---

## Core Concepts

### Identity
- Cryptographic identity using public and private keys
- Used for signing, encryption, and verification

---

### Sphere
A Sphere is a user’s local domain:
- Trust rules
- Credits
- Policies
- Services

Each user operates their own Sphere.

---

### Trust
- Local and directional
- Defines access, cost, and credit acceptance

---

### Neighbors
- Directly trusted peers
- Only neighbors can exchange credits directly

---

## Trust-Based Routing

Messages and value flow through trusted paths:

Alice → Bob → Carol → Dave


- No global broadcast
- No free reachability
- Routing follows trust relationships

---

## Peer Discovery

To avoid stagnation:

- Peers can announce themselves externally
- Discovery can happen through websites or directories

Important:
- Discovery does not imply trust
- Trust must be earned through behavior

---

## Resource Sharing Model

Everyone contributes:

- Storage for encrypted data
- Message routing
- Computation

System behavior:
- Reliable peers gain trust
- Unreliable peers lose trust
- Misbehavior can be shared

---

## MVP Goal

- Asynchronous messaging
- Messages persist even if peers are offline
- Delivery happens when routes become available

---

## Technical Overview

- Identity and signatures, Ed25519  
- Encryption, X25519 and XChaCha20-Poly1305  
- Hashing, BLAKE3 or SHA-256  
- Message discovery, Kademlia DHT  
- Routing, weighted graph algorithms such as Dijkstra  
- Storage verification, challenge-response proofs  

---

## Key Properties

- Anti-spam, cost increases with distance  
- Decentralization, no central control  
- Resource-based economy, value comes from contribution  
- Trust-scoped interaction, access is local  

---

## Status

Early concept and design phase.
