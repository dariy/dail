# Repository rulesets

`main.json` protects `main` (MISC-16): it cannot be deleted or force-pushed, changes arrive by pull request with
CI's checks green (the job names of `ci.yml`; the `ios` job counts as passed when its change filter skips it), and
only bypass actors may update it, so only repository admins can merge. The other bypass actor is the release App of
MISC-9 (`.github/workflows/bump.yml`), which pushes the version bump commit straight to `main`. Its ID is not
stored here; the apply command adds it.

`release-tags.json` guards the release tags: only admins and the release App may create, move or delete a `v*`
tag, so a tag push that starts a publishing `release.yml` comes from a merge (`bump.yml`) or an admin. The
`release` environment has no required reviewers: the PR merge is the approval.

Apply (create) either once the App exists, with the same command (`main.json` or `release-tags.json`); the App's ID
comes from the `release` environment's `RELEASE_APP_ID` variable:

```sh
app="$(gh variable get RELEASE_APP_ID --env release)"
jq --argjson app "$app" \
  '.bypass_actors += [{actor_type: "Integration", actor_id: $app, bypass_mode: "always"}]' \
  .github/rulesets/main.json | gh api --method POST repos/{owner}/{repo}/rulesets --input -
```

After a change to `main.json`, update it in place: `gh api repos/{owner}/{repo}/rulesets --jq '.[] | select(.name ==
"main") | .id'` gives the ID, and the same pipe goes to `gh api --method PUT repos/{owner}/{repo}/rulesets/<id>`.
When a job of `ci.yml` is renamed or added, change `required_status_checks` in the same PR.
