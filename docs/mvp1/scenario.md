for instance 1
set availability alwaysOn
set reachableAt www.example.com:9999
run instance 1
add service
```
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
}
```
run instance 2

connect to instance 1
asks for offerings
get
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
    }   
  ]
}
```