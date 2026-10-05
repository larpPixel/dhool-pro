# Gitea migration archive

Migrated from `jR4dh3y/dhool-pro` to `larpPixel/dhool-pro` on 2026-10-05.

This branch preserves exported repository metadata, discussions, PR details, reviews, releases, and all recorded status events. Original author identities, timestamps, merged status, and Gitea event history are retained in these records because the GitHub creation APIs cannot reproduce those fields. No changes were made to the original branches or tags.

Historical Gitea pull-request refs are mapped under `refs/gitea-import/` (or archival tags if GitHub rejects that namespace); see the manifest for exact mappings. They remain fetchable with Git.

CI runs and platform-specific state are preserved as exports/backups rather than executable GitHub Actions runs. GitHub Actions are disabled during the migration. Credentials and the Gitea server configuration are excluded from this branch.
