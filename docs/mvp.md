This is the basic scenario that must run.

Alice wants to send message to Bob
Alice has no public IP, Bob has no public IP
Bob told Alice that he has his connection data at CentralMessageHub.  

Bob send to Alice his unique id and the address for CentralMessageHub (www.isageek.com:9979)

Alice asks CentralMessageHub to list their services. She gets:  

```
[
  {"availability": "alwaysOn"},
  {"reachableAt": ["www.isageek.com.br:9979"]},
  {"operation":"buy", "creditType":"own", "price": 1, "type":"storage", "offerInfo":{"size": 100000, "period": 10, "maxRecoveryCalls": 1000}},
  {"operation":"sell", "creditType":"own", "price": 1,"type":"storage", "offerInfo":{"size": 100000, "period": 30, "maxRecoveryCalls": 1000}},
  {"operation":"sell", "creditType":"own", "price": 0.01,"type": "storeEncryptedPackage", "offerInfo":{"size": 1, "period": 10, "maxRecoveryCalls": 5}},
  {"operation":"sell", "creditType":"own", "price": 0.01,"type": "searchEncryptedPackage"},
  {"operation":"sell", "creditType":"own", "price": 0.01,"type": "getEncryptedPackage"},
  {"operation":"sell", "creditType":"own", "price": 0.0001,"type": "creditsBalance"},
  {"operation":"sell", "creditType":"own", "price": 0.001,"type": "lookupID"},
]
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
Alice is happy because she can now communicate with Bob, send him data or wathever.  
Bob is also Happy.  
CentralMessageHub is happy because they get storage. This is actually a redundant store they put in at least 5 nodes, ensuring any data almost 100% recovery chance. CentralMessageHub makes money by offering another unrelated service that uses the distributed storage.  