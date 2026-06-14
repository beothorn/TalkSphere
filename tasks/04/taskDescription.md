Because I am doing this project in small, contained steps I need to be able to call each function from the command line.  
In this task, you will redo the whole command line.  
The help needs to be very complete.
show with help h or --help

All command have an option dry-run that instead of doing the thing, print it on the stdout  

you can do it with --dry-run or d

Commands:

Chnage this command that already exists to this format:

talksphere run <listen_port> <peer_port> [home_folder]
talksphere config get home # print home folder
talksphere --help # prints all main commands (run, config, encryption, files, etc)

Some of these exists, some are new. Fix the existing and for the ones where the functionality does not exist yet, add placeholders.
At the end write a todo on the tasks/04 folder with all missing functionalities.

Encryption

talksphere encryption --help # prints help but only for encryption commands
talksphere encryption # same as help, prints help only for encryption commands
talksphere encryption create # Creates the keys on the home folder, error if keys exists
talksphere encryption recreate # override keys if exists, error if no key exists
talksphere --dry-run encryption create # Creates keys and show on stdout 
talksphere encryption encrypt_message "message" # Outputs encrypted messge to stdout
talksphere encryption sign_message "message" # Outputs messge signature to stdout

Files

talksphere files home # same as talksphere config get talksphere encryption

Ledger

talksphere ledger --help # prints help but only for ledger commands
talksphere ledger # same as help, prints help only for ledger commands
(I will not add this to all entries, but add a help for each command)
talksphere ledger credit_summary

Network

talksphere network ping <ip:port> # sends a message to ip port just to check if it is a talksphere server

Offerings

talksphere offerings <ip:port> # prints offering
talksphere offerings get # print my own offerings
talksphere offerings add <offering options>
talksphere offerings edit <offering options>
talksphere offerings remove <offering>

Shared storage

talksphere share local ls # lists all local files with meta info  
talksphere share remote ls # lists all remote files with meta info  
