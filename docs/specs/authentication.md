# Authentication and user data

## Ownership boundaries

- Rust derives the owner of personal reads and writes from authenticated identity.
  A segment ID, activity ID, query parameter, or UI selection never grants access
  to another athlete's personal results or source recordings.
- Cross-user reads are explicit product operations, such as saved overall segment
  standings. An overall result does not authorize access to the athlete's private
  activity details or recording samples.
- User-scoped features have multi-user tests using distinct identities, shared
  entities, unrelated records, and an athlete without personal results. Verify
  both positive access and exclusion of other users' data.

## Browser session data

- The API enforces authorization. The UI renders authorized personal responses
  and keeps those results separate from explicitly global standings.
- Bind cached application data to the authenticated user. On initial identity
  resolution, account change, or logout, cancel pending application queries and
  remove previous cached application data before rendering the new session.
  Keep the current-user query available to resolve the session boundary.
- While identity is loading or the cache belongs to another session, show a
  loading state instead of the previous session's data. Once signed out, protected
  content remains behind the existing authentication gate.
- Late responses from an earlier session must not repopulate the new session's
  personal data. Multiple assertions may verify this coherent isolation outcome.
- Successful logout cancels pending identity reads and marks the user signed out;
  the same session boundary clears cached application data. Failed logout leaves
  the current session intact.
