# Node layout

A Layout names one node root and derives the fixed paths inside it: `node.json`
(configuration), `public-id` (public identity text, so inspection needs no store lock), `identity/`, `board/`, `services/`, `ledger/`, `storage/`,
`catalog.json` (requester lease catalog), `operations.json` (recent control outcomes, including unknown ones) and `control.sock` (local control).
Computing a path never touches the filesystem. Every owner has its own directory,
so no module reads another module's private state.
