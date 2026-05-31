# TalkSphere Docs

TalkSphere is a peer-to-peer network built around these principles: 
- Decentralized  
- resource sharing  
- trust-based reachability  
- hardware independence  

Instead of relying on centralized platforms and servers, users contribute storage, computation, bandwidth, and routing capacity in a tit-for-tat economy, earning the ability to use resources provided by others.  

Communication is not globally free by default; the ability to reach someone depends on trust relationships and becomes more expensive as social distance increases, making spam and mass manipulation economically costly.  

Data is replicated across trusted peers, allowing users to access their information from any device without depending on a specific machine or cloud provider.  

By combining decentralized resource sharing with trust-based communication, TalkSphere aims to create a network where relevance emerges naturally from social relationships, infrastructure is collectively owned, and value is generated through contribution rather than advertising or attention extraction.  

# Resource sharing  

On Talksphere, every time you share a resource (storage or computation) you earn credits.  
Those credits are not global. If you allow Bob, for example, to use some of your storage, now Bob gives you one BobCredit.  
Of course, it all depends if Bob will honor the credits, but if he does, your trust on Bob goes up.  
This way, as trust grows, a network of available resources is created among trusted peers.  

# Trust based reachability  

The trust you have on your peers can also be used to determine who can reach you on social applications. If someone just want to reach you on social media, but is not going to give you anything in return, you will not be reached.  
This way, there is a much more direct relation between what you get from services than the current situation with ads.  

# Hardware independence

Since your data is spread among trusted peers, you can always get you data back as long as you have your private/public key pair.  

# Notes

Every user needs a random unique ID  
Every user needs keys for signing and a pair of keys for encrypting  

Needs to define Talksphere api and glossary

Identity: A person or a service denoted by a unique id
Challenge: Something that gives credits, can be keep something on storage, do some calculation or anything.
Credits: A debt between two identities. Both needs to keep track of credits owned/owed. Debts can be transferred, but it is a process that involves the three parties, new credit owner, old owner and the credit giver. A credit is always between two entities. 

Problem: who spends the credit, and when? It is like both sides have an infinite pile of credits
Credits cannot be twosided
When a service is provided, a decision is needed. Do I pay with the service provider credits or with mine?
The service provider may require their own credit. So for example
CentralMessageHub wants storage to be payed with their own credits
Alice wants to store a message, but CentralMessageHub does not want to get paid with Alice credits, but with CentralMessageHub credits
So, for Alice, it is ok to be paid either Alice credits or CentralMessageHub credits for her storage, but for CentralMessageHub, only CentralMessageHub counts

Unless there is some service Alice only accepts Alice credits, Alice credits are useless.


Messaging app
Dashboard app
Find people app
Public Identity app

Credit giving actions are open to anyone

Credit giving actions

Store data
Store and deliver package from Alice to Bob async (need identifying id and challenge keys)
Custom usage (credits for api usage)

Messaging app

The messaging uses TalkSphere to send/receive messages

Alice wants to send message to Bob
Alice has no public IP, Bob has no public IP
CentralMessageHub and AlternativeMessageHub have fixed public ip
Alice wants to use CentralMessageHub and AlternativeMessageHub
CentralMessageHub and AlternativeMessageHub offers some credit earning functions like storage per time per credit
Alice stores data for CentralMessageHub and AlternativeMessageHub and get AlternativeMessageHub credits
Alice signs a message (from her to Bob), encrypts it with Bob key so only he can read and sends it to CentralMessageHub and and AlternativeMessageHub using the credits she got from storing stuff. CentralMessageHub and AlternativeMessageHub promises to keep the message for the agreed time until Bob picks it
Bob search for messages for him in all his hubs (spending credits he got earlier)
CentralMessageHub tells Bob there is a message to him from Alice (all with uids, CentralMessageHub actually does not know Alice or Bob by name)
Bob spend someCentralMessageHub credits and recovers the message
Bob signs the message hash as delivered, so CentralMessageHub can tell Alice it was delivered next time she asks (with signed proof)  
When CentralMessageHub prove to Alice it delivered the message, Alice trust in CentralMessageHub grows.

The app that write and reads the message is not Talksphere, but talks to talksphere

Dashboard app

Similar, but CentralDashboard stores plain broadcast messages signed
Bob uses CentralDashboardCredits to store messages, and anyone that follows him can ask for the message (paying credits too)

Tit for tat storage
Bob and Alice  want to backup their data. They each Store each others data (Getting BobCredits and AliceCredits)
If Bob and Alice cant reach each other, they can use an intermediary, for example CentralMessageHub.
There can be a CentralMessageHub fast storage that is cheaper for live communication (short lived, costs less credits)


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
- Relevance-oriented, information flows through trusted paths  

