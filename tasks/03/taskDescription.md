We need that shared folder module be able to:  
Given a byte array, a id string, an owner id and an expiration, save the file somewhere on the home folder and keep track of the file info.

I am thinking about using sqlite to manage this.
I created two new folders, project/TalkSphere/linux/src/sharedStorage/fileSystem and project/TalkSphere/linux/src/sharedStorage/management to keep both logic separate.

Integration with the system is not needed, this just needs to be ready when the integration comes.  
This means good documentation.  
Acceptance Criteria:  
- Documentation (READMES, comments,logs)  
- Tests, Unit tests    
- Calling a function to store data  
- Calling a function to recover data from storage given id and owner id  
- Calling a function to delete a given entry (force delete)  
- Calling a function to clean up expired entries  
- Calling a function to run a SQL to query the file manager data  