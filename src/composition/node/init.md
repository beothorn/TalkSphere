# Node initialization and inspection

`initialize(root, config_document)` creates a fresh node and returns its public identity.
The configuration (or the defaults) is validated first, so an invalid document creates
nothing. The root must not exist (its parent must) or be an empty directory; a root with
any content is refused untouched. It then creates, in order: the owner-only root, the
protected identity, `node.json`, `public-id`, and every durable owner. If a later stage
fails, the partly created root stays for inspection and manual removal; nothing is
overwritten or silently repaired.

`describe(root)` reads the root without starting the node or taking any owner lock. It
reports the root, public identity, role (provider and requester, or requester only),
capacity, the number of cataloged leases, whether the state is initialized or incomplete
(naming missing parts) and whether the control socket exists. It fails only when the
root is not a directory.

`read_document(path)` reads an operator configuration file of at most 16384 bytes.
