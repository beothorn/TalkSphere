# Server lifecycle boundary

Application composition owns server startup and shutdown; Transport retains wire,
connection and stream state. Stage describes Configured, Starting, Ready, Stopping
and Stopped. Ready requires all startup prerequisites and successful binding;
Stopping refuses new work and drains accepted work to a finite deadline; Stopped
means owned resources have been released. These declarations alone do not run a
server or enforce transitions; the runtime implements them in subsequent steps.

Failure distinguishes Unsupported platform, Identity acquisition/validation,
Network setup, Runtime processing and ShutdownTimeout. These are local secret-free
categories, not protocol codes. No failure authorizes replacement keys or retries.
