Based on the mvp, change the offering to follow that format.  
 On files I see DEFAULT_OFFERINGS_TEXT
 There should not be hardcoded or defaul offering like that.
 Find a solution that has default offerings in a file, never hardcoded.
 Change the format.
 Then on socket connection, if the command is LIST_OFFERINGS, return the json string.
 If the offerings change on the file at home, this should reflect it