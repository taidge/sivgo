# Aggressive web production tasks

Goal: replace web OAuth/PWA/share/file stubs with production-shaped
implementations that have real boundaries and deterministic tests.

- [x] Replace OAuth mock exchange/userinfo with an injectable HTTP client,
  real token/user JSON parsing, state-aware auth URLs, and legacy request
  builders.
- [x] Replace the PWA stub service worker with a versioned offline/runtime cache
  strategy.
- [x] Replace deterministic share URLs with token-based share-link state,
  expiry/revocation metadata, and permission-aware links.
- [x] Replace no-op Web File API behavior with validated queued opens, saved
  files, downloads, and browser operation descriptors.
- [x] Add focused web tests for OAuth parsing, PWA service worker strategy,
  share token/revoke behavior, and file open/save/download behavior.
- [x] Run web tests and desktop check, then check the main `_todos.md` web item.
