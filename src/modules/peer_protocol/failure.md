# Failures and uncertain outcomes

Code preserves every specified version-one machine meaning; unknown remote strings
become Unknown, a generic service failure with no automatic retry. parse and wire
translate known spellings. classify maps typed local refusal, concealing access
as not_available when requested; Unknown effects cannot become an uncharged wire
failure. TermsChanged and PriceRejected remain explicit choices when the caller
knows the more precise context. error supplies a local category without replacing
the retained Code or interpreting diagnostic text.

Failure contains Code and optional untrusted UTF-8 diagnostic. parse decodes one
validated failure Message without financial effects. record emits only defined
codes and checks the1024-byte diagnostic bound before allocating controls. This
codec alone does not prove rollback. Keep diagnostics free of payloads/secrets;
never log peer text as trusted facts.

abort_provider requires receiving Invoke authority, original provider Association,
separate Services Coordinate/Observe grants and the correct local Coordinator.
It records/delivers an abort and confirms the final Aborted phase before returning
a failure suitable for this accepted exchange. Committed/uncertain outcomes or
failed delivery cannot claim uncharged failure. Quote/pre-acceptance refusals have
no admitted operation and use the bounded codec directly under exchange ordering.

ResultReceiver accepts failure before result.end and discards provisional bodies.
It retains diagnostics provisionally; only normal completion applies the original
provider's explicit failure via Coordinator.decline and acknowledged abort. A lost
END, CANCEL, truncation or disconnect retains Unknown and its original hold. Failure
after result.end is a protocol error and cannot replace successful output. No path
adds generic remote job-status, automatic work retry or rollback of committed work.
