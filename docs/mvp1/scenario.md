start an instance at a folder:
`talksphere --home /tmp/Alice start`
(NOT IMPLEMENETD, DEFAUL IS NOT EMPTY)  
```
talksphere --home /tmp/Alice  offerings get
{
}
``` 
add a credit for id 12321
`talksphere --home /tmp/Alice credit add 99 12321`
get availability  
```
talksphere --home /tmp/Alice config get availability
Config availability is not set
```
set availability alwaysOn
```
talksphere --home /tmp/Alice config set availability alwaysOn
Config availability set to alwaysOn
```
get reachableAt (NOT IMPLEMENTED)  
```
talksphere --home /tmp/Alice config get reachableAt
Config reachableAt is not set
```
set availability alwaysOn (NOT IMPLEMENTED)  
```
talksphere --home /tmp/Alice config set reachableAt www.example.com:9999
Config reachableAt set to alwaysOn
```

add service sell (NOT IMPLEMENTED)
```
talksphere --home /tmp/Alice  offerings add 
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
`talksphere --home /tmp/Bob start`

`talksphere --home /tmp/Alice run 9790 9791`
`talksphere --home /tmp/Bob run 9690 9691`

asks for offerings

`talksphere --home /tmp/Bob talk localhost:9790 GET_OFFERINGS`

ask for credits with id 12321  

`talksphere --home /tmp/Bob talk localhost:9790 GET_CREDITS 12321`



