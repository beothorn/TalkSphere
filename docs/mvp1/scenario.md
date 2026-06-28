for instance 1
start an instance at a folder:
`talksphere --home /tmp/Alice start`
add a credit for id 123456
set availability alwaysOn
set reachableAt www.example.com:9999
run instance 1
add service sell
```
{
  "service": "creditWithdraw", 
  "credit":"own", 
  "description": "given a code, withadraw credits",
  "input": {
    "code": "string"
  },
  "output": {
    "success": "bool"
  }
}
```
run instance 2

connect instance 1 to instance 2
asks for offerings
get
```
{
  "availability": "alwaysOn",
  "reachableAt": ["www.isageek.com.br:9979"],
  "sell" : [
    {
      "service": "creditWithdraw", 
      "credit":"own", 
      "description": "given a code, withadraw credits",
      "input": {
        "code": "string"
      },
      "output": {
        "success": "bool"
      }
    }   
  ]
}
```

ask for credits with id 123456  
get credits  
