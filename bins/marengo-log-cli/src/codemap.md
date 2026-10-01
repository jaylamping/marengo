# bins/marengo-log-cli/src/

## Responsibility
`main.rs` parses and dispatches session, archive, purge, legacy/journal import and
disk-usage commands to `marengo-store`. Candump inspection and explicit
`recover-known-v2` dispatch before normal Store open. Recovery prints the library's
completed JSON receipt; schema recognition and backup/publication belong to the library.
