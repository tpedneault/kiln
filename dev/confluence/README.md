# Confluence instances for Kiln's live tests

Kiln's own tests talk to an in-process fake Confluence; nothing in CI
reaches a real one. The live tests (`crates/kiln/tests/live.rs`, marked
`#[ignore]`) publish a small project into a real space to confirm what
the fake can't: the endpoints marked **verify** in
[CONFLUENCE.md](../../docs/CONFLUENCE.md), storage as each flavour stores
it, drift after a real edit. Run them before a release, and whenever the
client changes, against both flavours.

## What the tests need

| Variable | Value |
|---|---|
| `KILN_TEST_CONFLUENCE_URL` | the base URL: `https://<site>.atlassian.net/wiki`, or `http://localhost:8090` |
| `KILN_TEST_CONFLUENCE_SPACE` | a space key the token can write to, say `KILNTEST` |
| `KILN_TEST_CONFLUENCE_PARENT` | the id of a scratch page in it; every run creates its pages below |
| `KILN_TEST_CONFLUENCE_TOKEN` | Cloud: an API token. Data Center: a personal access token |
| `KILN_TEST_CONFLUENCE_USER` | Cloud only: the account e-mail the API token belongs to |
| `KILN_TEST_CONFLUENCE_FLAVOUR` | optional: `cloud` or `datacenter` (by default `*.atlassian.net` is Cloud) |

```sh
cargo test -p kiln --test live -- --ignored --test-threads 1
```

Each run's pages carry the run's time in their titles, so runs don't
collide and a failed run leaves its pages to look at. Delete the scratch
page's children by hand now and then (or delete the page and make a new
one: its id goes in `KILN_TEST_CONFLUENCE_PARENT`).

A page's id is in its URL (`…/pages/123456789/…`), or under "⋯ → Page
information" (`pageId=` in the address).

## Cloud: a free site

1. Sign up for Confluence Cloud Free at
   <https://www.atlassian.com/software/confluence/free> (up to 10 users).
   The site is `https://<name>.atlassian.net/wiki`.
2. Create a space (key `KILNTEST`) and a page in it named "Kiln scratch".
3. Create an API token at
   <https://id.atlassian.com/manage-profile/security/api-tokens>. Tokens
   with scopes need read and write access to Confluence pages,
   attachments, labels and content properties.
4. Set the variables above, with your account e-mail as `_USER`.

## Data Center: the `atlassian/confluence` image

Atlassian publishes Confluence Data Center as a container image and
issues free **timebomb** licences for testing (valid for a few hours to
days; see "Timebomb licenses for testing server apps" on
developer.atlassian.com). Confluence needs a database; PostgreSQL in a
second container is the usual pairing.

```sh
docker network create kiln-confluence
docker run -d --name kiln-confluence-db --network kiln-confluence \
  -e POSTGRES_USER=confluence -e POSTGRES_PASSWORD=confluence -e POSTGRES_DB=confluence \
  postgres:15
docker run -d --name kiln-confluence --network kiln-confluence -p 8090:8090 \
  -e JVM_MINIMUM_MEMORY=1024m -e JVM_MAXIMUM_MEMORY=2048m \
  atlassian/confluence:8.5
```

Then, at <http://localhost:8090>:

1. Choose a production installation, paste a timebomb licence (the 10
   user Confluence Data Center one) when asked for a key.
2. Database: PostgreSQL, host `kiln-confluence-db`, port 5432, database,
   user and password `confluence`.
3. Create the administrator, then a space `KILNTEST` and a page
   "Kiln scratch" in it.
4. Profile → Settings → Personal Access Tokens → Create token (7.9 and
   later). That is `KILN_TEST_CONFLUENCE_TOKEN`; leave `_USER` unset.

The first start takes several minutes and about 2 GB of memory. When the
licence runs out, paste a fresh timebomb licence under General
Configuration → License Details; the data stays.

To stop and remove everything:

```sh
docker rm -f kiln-confluence kiln-confluence-db && docker network rm kiln-confluence
```

## What the live tests check, and what they don't

They publish each layout and publish again (expecting no change), edit a
page through the API (expecting drift, then `--force`), and remove an item
(expecting its page to retire). They don't judge how pages look: anchors,
the table of contents, diagram legibility and how a large page edits are
for a person, listed under "Needs checking" in the pull request that
changes them. Record what an instance did in CONFLUENCE.md next to the
**verify** it settles, with the flavour, version and date.
