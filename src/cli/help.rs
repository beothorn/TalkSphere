//! Shared concise overview and discoverable per-command/option help definitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Topic {
    Overview,
    Help,
    Version,
    Serve,
    Identity,
    Board,
    Ledger,
    Services,
    PeerProtocol,
    Storage,
    Messaging,
    Pages,
    Node,
    NodeCtl,
    Policy,
    ServerOptions,
    Logging,
}

pub(super) struct HelpTopic {
    pub topic: Topic,
    pub names: &'static [&'static str],
    label: &'static str,
    summary: &'static str,
    details: &'static str,
}

// The overview and detailed help share these definitions. Every new command
// or option needs a topic here and corresponding parser/behavior tests.
pub(super) const TOPICS: &[HelpTopic] = &[
    HelpTopic {
        topic: Topic::NodeCtl,
        names: &[
            "node-ctl",
            "--peer",
            "--amount",
            "--request-id",
            "--millionths",
            "--file",
            "--retention-seconds",
            "--max-spend",
            "--lease",
            "--provider",
            "--offset",
            "--length",
            "--output",
            "--seconds",
            "--expected-expiry",
            "--expected-file",
            "--expected-byte",
            "--unknown",
        ],
        label: "node-ctl",
        summary: "Control a running node: credits, trust, paid Storage and inspection.",
        details: "Usage: talksphere node-ctl --node-root DIRECTORY ACTION [--option VALUE]...\n\nSends one request to the owner-only control socket of the node started with serve --node-root and prints the reply as JSON. The socket is not reachable from the network and is not the CR2SE Node API. Exit 0 on success; 1 on a node refusal or an unreachable node (stderr shows code: message); 2 on invalid syntax.\nActions:\n  status                          identity, address, role, lease count, unknown outcomes, balances at known providers, and live authenticated connections (count and distinct peer IDs)\n  leases                          cataloged leases with expiry and state\n  outcomes [--unknown]            recent operation outcomes; --unknown lists only operations whose remote outcome is unknown\n  grant --peer ID --amount N [--request-id N]   issue N finite credits to a peer once per request id; resending the same id repeats the reply and never grants twice while the node runs\n  own --peer ID --amount N        record the credits you hold from that issuer (absolute value)\n  trust --peer ID --millionths N  set exact local trust, 0 to 1000000\n  balance --peer ID               issuer-separated owned, issued and trust for that peer\n  fetch --contact CONTACT         authenticate a contact (ID?address=IP&port=PORT, or just the ID for a peer that has a live connection to this node, such as a client that connected outward) and show its provided offerings\n  fulfil --contact CONTACT        offer to fulfil the peer's wanted Storage terms: the peer chooses one queued object, this node holds it as a provider and earns the peer's credits (trust the peer first); nothing is spent\n  wanted-add --file FILE          queue a nonempty object (within the wanted terms) for the next client that fulfils this node's wanted offering; needs credit.wanted in policy.json\n  policy-show                     the validated peer policy parameters and node-wide counters (bootstrap credits issued, live connections, wanted queue)\n  policy-explain --peer ID        what each policy decision would be for that peer now (admission, limits, trust after each verified event, bootstrap) and the local observations behind it; never any peer-supplied text\n  replica-add --file FILE         seal a hosted item (nonempty, at most 16343 bytes) with the node's data key and add it to the replica book; contributing clients each receive it until the redundancy target is met; needs credit.wanted in policy.json\n  replicas                        list hosted items with the CR2SE redundancy state (not redundant, under-replicated, target satisfied), the local target and the unexpired confirmed copies shown separately, whether the item is at risk, stale observations and each replica's expiry\n  store --contact CONTACT --file FILE --retention-seconds N --max-spend N   store a nonempty file of at most 16384 bytes and catalog the validated lease\n  retrieve --lease HEX --max-spend N [--provider ID] [--offset N] [--length N] [--output FILE]   retrieve a range; with --output the validated bytes go to a new owner-only file\n  renew --lease HEX --seconds N --max-spend N [--provider ID] [--expected-expiry N]   extend a lease; the expected expiry defaults to the cataloged one\n  check --lease HEX --max-spend N (--expected-file FILE [--offset N] | --expected-byte HEX --offset N) [--provider ID]   paid one-byte check; expected bytes stay local and a missing offset is sampled securely\n  remove --lease HEX --max-spend N [--provider ID]   charged logical removal\nA contact given as an ID alone is reached over its live authenticated connection, whichever side opened it; with no live connection and no address the request fails at once without charge. Spending requires an explicit --max-spend ceiling in credits; the exact quoted price is charged. --provider is needed only when a lease id exists at several providers. A lost reply is reported as unknown_outcome and recorded; it is never retried and never replaced by a new store. Option values must be nonempty and options may not repeat. See plans/10_Minimum_node/PLAN.md.\n",
    },
    HelpTopic {
        topic: Topic::Node,
        names: &["node-init", "node-show", "--node-root", "--config"],
        label: "node-init | node-show",
        summary: "Create or inspect a minimum useful node root.",
        details: "Usage: talksphere node-init --node-root DIRECTORY [--config FILE] | talksphere node-show --node-root DIRECTORY\n\nnode-init creates a fresh owner-only node root with its own protected identity and every durable owner (Board, Services, Ledger, Storage, lease catalog). The optional --config FILE is a node.json document (version 1; optional offering, capacity, credits and control members); omitted members use finite defaults and an explicit \"offering\": null makes a requester-only node. The document is validated before anything is created. An existing root with content is refused untouched; a failure after creation leaves the partial root for inspection. Output is only the public CR2SE ID.\nnode-show reads the root without starting it or taking any lock and reports identity, role, capacity, cataloged leases, the peer policy state (defaults, valid or invalid policy.json), initialized or incomplete state and whether the control socket exists.\nAn optional policy.json in the node root sets peer policy parameters (version 1; admission, quota, trust, credit, redundancy and connection members, see docs/policy/PARAMETERS.md); omitted members keep defaults that reproduce the minimum node, and serve --node-root refuses to start on an invalid file.\nInvalid syntax exits with status 2; runtime failures exit 1 and explain the problem on stderr; success and help exit 0. See plans/10_Minimum_node/PLAN.md.\n",
    },
    HelpTopic {
        topic: Topic::Policy,
        names: &["policy", "policy.json", "policy-sim", "policy_sim"],
        label: "help policy",
        summary: "Peer policy: policy.json, inspection and the simulation.",
        details: "Usage: talksphere help policy\n\nCR2SE leaves trust, credit bootstrap, quotas, redundancy and connection upkeep to each identity. An optional policy.json in a node root (version 1; admission, quota, trust, credit, redundancy and connection members) sets them; omitted members keep defaults that reproduce the minimum node, an invalid file stops serve --node-root, and node-show reports its state. Every member, default and range is in docs/policy/PARAMETERS.md.\nnode-ctl policy-show prints the validated parameters; policy-explain --peer ID shows the local observations about a peer and what each decision would be (see help node-ctl). A node with credit.wanted publishes a wanted Storage offering, pays clients in its own credits for holding sealed copies of hosted items (node-ctl replica-add, replicas, wanted-add; clients run fulfil) and keeps them checked and renewed over the clients' own connections.\nThe simulation compares parameters under a modelled unreliable client fleet: cargo run --locked --release --example policy_sim -- help sweep. It runs the real policy and replica code against a modelled network and prints figures under stated assumptions, recorded in docs/policy/SIMULATIONS.md. Example help and success exit with status 0, runtime failures 1 and invalid arguments 2. See plans/12_Peer_policy/PLAN.md.\n",
    },
    HelpTopic {
        topic: Topic::Storage,
        names: &["storage"],
        label: "help storage",
        summary: "Exercise native paid Storage over authenticated loopback.",
        details: "Usage: talksphere help storage\n\nRun cargo run --locked --example storage -- run on Linux. Creates private temporary owners and authenticated loopback listeners. Stores 5000 bytes for 60 seconds, reopens the provider's native journals, then retrieves, checks, renews and removes the lease. Each lease represents one independently owned copy. RUST_LOG controls stderr logging. Detailed example help is cargo run --locked --example storage -- help run. Help exits with status 0. Example failures use status 1 and invalid syntax uses status 2. See docs/storage/EXERCISE.md.\n",
    },
    HelpTopic {
        topic: Topic::Messaging,
        names: &["messaging"],
        label: "help messaging",
        summary: "Exercise paid Messaging with replicas over authenticated loopback.",
        details: "Usage: talksphere help messaging\n\nRun cargo run --locked --example messaging -- run on Linux. Creates four authenticated loopback nodes with private temporary protected identities and independent native stores: a sender, a recipient and two providers. The sender encrypts one message to the recipient's published key, signs it and stores a replica at each provider at its exact quoted price. The recipient discovers it at both providers, recovers it with a signed request, decrypts it and receives it once. The sender then reads the recovery receipt, renews a placement, runs one paid byte check on an unread message and removes it, releasing credits exactly once. One placement is one copy at one provider; redundancy is the sender's choice. Nothing is advertised by default serve or node-init; the module is exercised through its Services provider and Peer Protocol requester plans. RUST_LOG controls stderr logging. Detailed example help is cargo run --locked --example messaging -- help run. Help exits with status 0, example failures use status 1 and invalid syntax uses status 2. See docs/messaging/EXERCISE.md.\n",
    },
    HelpTopic {
        topic: Topic::Pages,
        names: &["pages", "hosted-publications"],
        label: "help pages",
        summary: "Exercise hosted pages: a publisher that never accepts connections and two hosts.",
        details: "Usage: talksphere help pages\n\nRun cargo run --locked --example pages -- run on Linux. Creates four authenticated loopback nodes with private temporary protected identities and independent native stores: two hosts, a publisher that only dials out (like a phone that cannot accept connections) and a reader. The publisher signs one public page snapshot and stores the same signed publications at both hosts at their exact quoted prices. While the publisher is offline the reader lists, fetches and verifies the page, then again from one host alone. The publisher then stores a newer page encrypted to the reader's published key at one host; the reader opens it, a reader without the key learns nothing and across both hosts the reader takes the best-known (never proven latest) version. The publisher renews a placement, runs one paid byte check and removes the old page, which releases credits exactly once. One placement is one copy at one host; redundancy is the publisher's choice. The publisher-served cr2se.page service and the hosted publication service are exercised through their Services providers and Peer Protocol requester plans; nothing is advertised by default serve or node-init. RUST_LOG controls stderr logging. Detailed example help is cargo run --locked --example pages -- help run. Help exits with status 0, example failures use status 1 and invalid syntax uses status 2. See docs/pages/EXERCISE.md.\n",
    },
    HelpTopic {
        topic: Topic::PeerProtocol,
        names: &["peer-protocol"],
        label: "help peer-protocol",
        summary: "Exercise authenticated custom work and inspect Peer Protocol configuration.",
        details: "Usage: talksphere help peer-protocol | cargo run --locked --example peer -- run\n\nThe explicit two-node example uses real protected Identity, Cryptography, TCP, Board, Services and Ledger; price=7 and both remaining balances=93. Service/accounting state is volatile and cannot resume. It installs only a custom text handler; paid Storage and optional families remain separate modules. Default serve still registers no application protocol.\nExample overview: cargo run --locked --example peer -- --help; detailed configuration: cargo run --locked --example peer -- help run. No arguments show example help successfully. run is explicit permission to create temporary protected credentials and ephemeral loopback listeners; no operator store is opened.\nLibrary configuration separates Bridge framing/workers, Endpoint aggregate admission, independently installed handlers and owner authorities; absent claim keys, checks or wanted policy return unsupported. Synchronous dependencies execute on bounded workers. Durable owner recovery never retries unknown work automatically. RUST_LOG selects stderr target/level filters; payloads and private keys are excluded.\nHelp/success exits with status 0; invalid syntax exits 2; example runtime/configuration failures exit 1. See docs/peer-protocol/RUNTIME.md, docs/peer-protocol/NODES.md and plans/8_Peer_Protocol_module/PLAN.md.\n",
    },
    HelpTopic {
        topic: Topic::Services,
        names: &["services"],
        label: "help services",
        summary: "Exercise the local Services library with a finite custom handler.",
        details: "Usage: talksphere help services | cargo run --locked --example services\n\nThe explicit local demonstration registers a custom text handler and uses real Board and Ledger contracts; it prints price=7, remaining=93. It uses volatile stores and no sockets, keys, standard-family runtime or automatic retries. Every run starts a new demonstration; it cannot resume after process exit.\nLibrary defaults: 4096 handlers/operations, 65536 bodies/nodes, 64 quotes and 16777216 state bytes. Terminal history is retained; full capacity refuses new work. Production Linux recovery reopens protected participant stores under fresh grants and resumes original decisions, never execution.\nExample --help/-h exits with status 0; invalid example arguments exit 2 and runtime failures exit 1. This help exits 0. Production serve still registers no application protocol.\nSee src/modules/services/README.md, ACCEPTANCE.md and plans/7_Services_module/PLAN.md.\n",
    },
    HelpTopic {
        topic: Topic::Ledger,
        names: &[
            "ledger-show",
            "--ledger-store",
            "--local-identity",
            "--account",
        ],
        label: "ledger-show",
        summary: "Inspect issuer-separated accounting in an existing protected Ledger.",
        details: "Usage: talksphere ledger-show --ledger-store DIRECTORY --local-identity CR2SE_ID --account CR2SE_ID\n\nAll three options are required in the displayed order. --ledger-store selects an existing owner-only Linux Ledger directory; --local-identity is its exact expected owner; --account selects one external issuer/owner account.\nOutputs exact integer balances, available credits, trust in millionths and aggregate participant/payment statuses; currencies remain separate. Local inspection limits are 4096 accounts per view, 4096 operations, 65536 transfers/claims and 16777216 state bytes; quote capacity defaults to 64.\nInspection takes the exclusive writer lock and validates complete state; missing/corrupt/insecure stores, contention or unsupported platforms exit 1 without initializing or repairing accounting. Invalid syntax exits 2; success/help exits 0. No secrets, grants, claims, network calls or new spending are produced.\nSee plans/6_Ledger_module/PLAN.md and src/modules/ledger/README.md.\n",
    },
    HelpTopic {
        topic: Topic::Board,
        names: &[
            "board-validate",
            "board-show",
            "--board-file",
            "--board-store",
            "--publisher",
        ],
        label: "board-validate | board-show",
        summary: "Validate a public Board file or inspect a protected snapshot.",
        details: "Usage: talksphere board-validate --board-file FILE | talksphere board-show --board-store DIRECTORY --publisher CR2SE_ID\n\n--board-file selects one public UTF-8 JSON document, bounded to 1048576 bytes and depth 31.\nValidation checks Board format and exact prices, not separately supplied definitions or local publishability.\n--board-store selects an existing owner-only Linux Board directory, separate from the identity store.\n--publisher supplies its exact expected CR2SE identity; mismatch is refused. Inspection locks and validates stored documents, then prints original public Board JSON.\nNo command publishes offerings, registers services, spends credits or opens a network connection.\nInvalid syntax exits with status 2; invalid/missing/corrupt documents, contention and unsupported platforms exit with status 1; success exits with status 0.\nSee plans/5_Board_module/PLAN.md and src/modules/board/README.md.\n",
    },
    HelpTopic {
        topic: Topic::Logging,
        names: &["logging", "RUST_LOG"],
        label: "RUST_LOG",
        summary: "Filter structured operational logs on stderr.",
        details: "Usage: RUST_LOG=info talksphere serve --identity-store DIRECTORY\nRUST_LOG defaults to info; off disables logs; target filters such as talksphere=trace enable diagnosis.\nOperational logs use stderr; stdout contains intentional public CLI results. Never include secrets in diagnostics.\nHelp/version ignore logging configuration and exit with status 0; malformed filters prevent runtime startup with status 1.\n",
    },
    HelpTopic {
        topic: Topic::ServerOptions,
        names: &[
            "--listen",
            "--contact",
            "--stored-contacts",
            "--max-connections",
            "--max-streams",
            "--outbound-bytes",
            "--buffered-bytes",
            "--handshake-ms",
            "--shutdown-ms",
        ],
        label: "serve options",
        summary: "Configure listening, contacts, limits and shutdown.",
        details: "Usage: talksphere serve --identity-store DIRECTORY [OPTION VALUE]...\n\n--listen IP:PORT defaults to 127.0.0.1:0; zero selects an assigned listening port.\n--contact CONTACT repeats up to 64 times; requires a canonical ID with an IP/port, no Discovery.\nWith serve --node-root each contact is kept connected: a lost or failed connection is retried with a doubling delay from one second up to connection.reconnectMaxSeconds of policy.json (default 60), and every live connection is pinged every connection.pingSeconds (default 30) and closed when a pong takes longer than connection.pingTimeoutSeconds (default 10). Plain serve --identity-store tries each contact once.\n--stored-contacts yes|no defaults to no; yes selects the protected snapshot instead of explicit contacts.\n--max-connections COUNT defaults to 64; --max-streams COUNT defaults to 128 per connection.\n--outbound-bytes BYTES defaults to 4196096; --buffered-bytes BYTES defaults to 65536 per stream.\n--handshake-ms MILLISECONDS defaults to 10000; --shutdown-ms MILLISECONDS defaults to 5000.\nBudgets must support maximum protocol frames, durations must be positive; duplicate non-contact options are invalid.\nOptions are validated before I/O; invalid arguments exit with status 2. SIGINT/SIGTERM stop admission and drain to --shutdown-ms; a repeated signal forces closure.\n",
    },
    HelpTopic {
        topic: Topic::Identity,
        names: &["identity-init", "identity-show", "--identity-store"],
        label: "identity-init | identity-show",
        summary: "Create or inspect a protected local identity.",
        details: "Usage: talksphere identity-init --identity-store DIRECTORY | talksphere identity-show --identity-store DIRECTORY\n\nExplicit initialization installs both key pairs without overwrite; inspection never generates keys.\n--identity-store requires an explicit owner-only Linux directory; parents must exist.\nThe same --identity-store option is required by serve, which only loads existing keys.\nOutput from identity administration is only the public canonical CR2SE ID. Files are permission-protected, not encrypted.\nMissing/corrupt/insecure stores and conflicting initialization exit with status 1; invalid syntax exits with status 2.\nHelp exits with status 0. See plans/4_Transport_integration/PLAN.md.\n",
    },
    HelpTopic {
        topic: Topic::Serve,
        names: &["serve"],
        label: "serve",
        summary: "Run authenticated Transport connections (Linux).",
        details: "Usage: talksphere serve (--identity-store DIRECTORY | --node-root DIRECTORY) [OPTION VALUE]...\n\nRequesting help opens no sockets and exits with status 0.\nLoads existing protected keys; missing or inconsistent credentials never generate replacements.\nReports ready <PUBLIC_ID> <ACTUAL_ADDRESS> and verified authenticated peer IDs.\nFrame payloads remain visible; application encryption and services are separate modules.\nWithout --node-root, production registers no application protocol and unsupported OPEN requests are refused.\nWith --node-root DIRECTORY (instead of --identity-store) serve runs the minimum useful node created by node-init: it opens and recovers every durable owner, publishes the configured Storage offering, serves Peer Protocol on authenticated connections and accepts local node-ctl requests on the owner-only control.sock. --stored-contacts is not supported with --node-root; SIGINT/SIGTERM let accepted work drain before stopping. See help node-init and help node-ctl.\nUse talksphere help --listen for contacts, defaults, limits and timeouts.\nSIGINT/SIGTERM stop admission and drain; repeated signals force remaining connections closed.\nInvalid syntax exits with status 2, startup/runtime failure with status 1, normal shutdown with status 0.\nSee plans/4_Transport_integration/PLAN.md and docs/transport-integration/ACCEPTANCE.md.\n",
    },
    HelpTopic {
        topic: Topic::Help,
        names: &["help", "--help", "-h"],
        label: "help [TOPIC], -h, --help",
        summary: "Show the overview or detailed topic help.",
        details: "Usage: talksphere --help | talksphere -h | talksphere help [TOPIC]\n\n\
                  With no arguments, or with --help, -h or help alone, show the overview.\n\
                  Use 'talksphere help <topic>' for details on any command or option.\n\
                  Topics accept option spellings: 'talksphere help --version' and\n\
                  'talksphere help version' show the same details.\n\
                  Use 'talksphere serve --help' for command-specific help.\n\
                  Help exits with status 0 and does not start the server. Unknown topics\n\
                  and unsupported argument combinations exit with status 2.\n",
    },
    HelpTopic {
        topic: Topic::Version,
        names: &["version", "--version", "-V"],
        label: "-V, --version",
        summary: "Print the TalkSphere application version.",
        details: "Usage: talksphere --version | talksphere -V\n\n\
                  Print 'talksphere <version>' from the Cargo package metadata and exit\n\
                  with status 0. This is the application version, not a CR2SE protocol\n\
                  version or a claim of conformance. The server is not started.\n\
                  This option takes no value and must be used on its own.\n",
    },
];

// Resolve documented aliases without guessing unsupported options.
pub(super) fn find_topic(name: &str) -> Option<Topic> {
    tracing::trace!(function = "help::find_topic", "Resolving a help topic");
    TOPICS
        .iter()
        .find(|entry| entry.names.contains(&name))
        .map(|entry| entry.topic)
}

// Render the shared overview or full topic with usage and exit behavior.
pub(super) fn render(topic: Topic) -> String {
    tracing::trace!(function = "help::render", "Rendering discoverable CLI help");
    if let Some(entry) = TOPICS.iter().find(|entry| entry.topic == topic) {
        return format!("{}\n\n{}", entry.summary, entry.details);
    }

    let mut overview = format!(
        "TalkSphere {} - Rust server and proof of concept for CR2SE\n\
         Protocol: https://github.com/beothorn/CR2SE\n\
         Status: authenticated Transport server available on Linux; Peer Protocol library and explicit custom-node example available; minimum useful paid Storage node via node-init, serve --node-root and node-ctl.\n\n\
         Usage: talksphere [COMMAND | OPTION]\n\n\
         Commands and options:\n",
        env!("CARGO_PKG_VERSION")
    );
    for entry in TOPICS {
        overview.push_str(&format!("  {:<28} {}\n", entry.label, entry.summary));
    }
    let names: Vec<_> = TOPICS.iter().map(|entry| entry.names[0]).collect();
    overview.push_str(&format!(
        "\nDetailed help: talksphere help <topic>\n\
         Topics: {} (option spellings also work).\n\
         Examples: talksphere help serve | talksphere help --version\n",
        names.join(", ")
    ));
    overview
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overview_lists_all_topics_and_how_to_get_details() {
        let overview = render(Topic::Overview);
        assert!(overview.contains("talksphere help <topic>"));
        assert!(overview.contains(
            "authenticated Transport server available on Linux; Peer Protocol library and explicit custom-node example available; minimum useful paid Storage node via node-init, serve --node-root and node-ctl"
        ));
        for entry in TOPICS {
            assert!(overview.contains(entry.label));
            assert!(overview.contains(entry.summary));
        }
    }

    // Serve help distinguishes configured authenticated startup from a missing identity selection.
    #[test]
    fn serve_help_explains_protected_startup_and_integration_evidence() {
        let details = render(Topic::Serve);
        assert!(details.contains("existing protected keys"));
        assert!(details.contains("plans/4_Transport_integration/PLAN.md"));
        assert!(details.contains("ACCEPTANCE.md"));
        assert!(details.contains("never generate replacements"));
        assert!(details.contains("unsupported OPEN"));
    }

    // Every detailed topic preserves discoverable usage and exit behavior.
    #[test]
    fn detailed_topics_explain_usage_and_exit_behavior() {
        for entry in TOPICS {
            let details = render(entry.topic);
            assert!(details.contains("Usage:"));
            assert!(details.contains("status"));
            assert!(details.len() > entry.summary.len());
        }
    }
}
