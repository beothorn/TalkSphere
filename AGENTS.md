Variables names should not be abbreviated  
Parameters should have their own line on function signature. Example:  
```
static int parse_port(
    const char *text, 
    const char *name, 
    int *port
) {
```  
Variable name refers to what the variable is, not what it should do. Example:  
Prefer `server_address` over `server_address_to_connect_after_starting`  

Comment on the variable. Example:  
```
server_file_descriptor = socket(
    AF_INET, // AF_INET = IPv4
    SOCK_STREAM, // SOCK_STREAM = TCP stream socket.
    DEFAULT_SOCKET_PROTOCOL
);
```

No magic numbers, replace all literal numbers with constants.  


Prefer breaking down function instead of commenting.  
Prefer to put functions related to the same responsability in a separate file.  
Use folders to organize code, try to organize by domain instead of technology. Example:  
Prefer:  
```
resource_sharing
  network
    message_logic
trusted_network
  network
    message_logic
```
over
``` 
network
  message_logic
    resource_sharing
    trusted_network
```  

Keep in mind separation of responsibilities. Do not share data structures between unrelated domains.  
For example, if socket has a struct package, it may be convenient to use package on encryption, but that would violate separation of responsibilities.  
Prefer instead using many primitive arguments or a structure in a shared parent folder.  
It is extremely important to not mix responsibilities, so each module can be understood and tested in isolation.  
This also helps any implementation to deal with less code. Keep responsibilities contained.  
This also applies inside modules. If it is possible to break responsibilities down into more folders, do it.  

Rule of thumb, if include has a parent folder, it should not have a folder after it. Example:  
Good:
`#include "../logging.h"`  
Bad:
`#include "../program_arguments/arguments.h`  
Okayish (as long as it is clear what it contains and it only contains stuff shared by at least two folders):  
`#include "../common/result.h"`  
Why? because having one folder knowing the contents of another creates strong dependency. We need to separate concerns to make it maintenable.  

Prefer to reassign a variable to a better named variable than reusing it. Example:  
Prefer:
```
const char *program_name = argv[PROGRAM_NAME_ARG_INDEX];
usage(program_name);
```
over
```
usage(argv[PROGRAM_NAME_ARG_INDEX]);
```  

The comments should be formatted as if you where having a conversation explaining to a junior dev what a code does. 
Those are examples, feel free to vary on this:

```
    In this function we ...
    This class is responsible for ...
    This concerns ... from ...
    ...this is required because it is called from class ...
    This is a proxy ...
    This delegates ...
    This represents ...
    This is faster because ...
    This uses the data structure .... in this case because ...
    This function should ... and it does it by ..
    The way this function works is ... and it is needed because ... so we return ...
    Here we simply ... and return ...
    This class is resposible for ... this should not do x because tis is the responsability of class ...
    On this class we have all ... because having them together makes it easier to find an maintain ...
    Here we isolate the logic for ... so we don't have it on ...
    This function could be joined with ... but I decided not to do it because ... to maintain ...
    This is here to isolate code responsible for ...
    The way this works is ...
    ... and pay attention because the behavior here is ...
    ... is unusual because we need to avoid ....
```
And so on...

Always add logs, always add a trace log at the start of a function.  
Add only few info logs, only for important events or big logic branching.  

For all else, follow the decision chain:
For who am I writing the log? 
- If for Devs: Do I need t log variables? If yes is a debug, if not it is a trace
If writing logs to system operators:
- Do I log because of unwanted or unexected state? If no, it is info
- Can the process continue with the unwanted or unexected state? If yes, use warn
- Can the application continue with the unwanted or unexected state? If yes, use error, if not use fatal

About the levels:
- info: Use sparsingly, only for inportant events
- warn: Explain why the state is unwanted or unexected  
- error: Explain why the state is unwanted or unexected and consequences  
- trace: Use a lot, basically as frequent as comments, should contain the funcion, format as a narrative such as, "foo(int a, float b): now this happens with a" or "bar(): after we have value y, we do xyz to get the zyx" and so on...  
- debug: Use a lot, format as a sentence such as, "Sending int x to database" or "Mapping x to y" and so on...

Basically, info should inform me the ap is running ok, trace should give me a narraive that describe the whole flow of data and debug should besides all that give me values so I can figure out why things happens with the given values.  

Use commmon log engine, configure it so it knows the class the looger is being called from and it is easy to filter.

Always add tests. Add tests for the happy path and for errors. 
Always ask your self, what could go wrong here, then add a test to ensure the app has a good behavior.

Remember, DO NOT LEAVE TESTS FAILING, FIX IT! 
