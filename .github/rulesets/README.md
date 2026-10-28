# Repository rulesets

`main.json` protects `main` (MISC-16): it cannot be deleted or force-pushed, changes arrive by pull request with
CI's checks green (the job names of `ci.yml`; the `ios` job counts as passed when its change filter skips it), and
only bypass actors may update it, so only repository admins can merge. The other bypass actor is the release App of
MISC-9 (`.github/workflows/bump.yml`), which pushes the version bump commit straight to `main`. Its ID is not
stored here; the apply command adds it.

Apply (create) it once the App exists, with `DAIL_RELEASE_APP_ID` set to the App's ID:

```sh
jq --argjson app "$DAIL_RELEASE_APP_ID" \
  '.bypass_actors += [{actor_type: "Integration", actor_id: $app, bypass_mode: "always"}]' \
  .github/rulesets/main.json | gh api --method POST repos/{owner}/{repo}/rulesets --input -
```

After a change to `main.json`, update it in place: `gh api repos/{owner}/{repo}/rulesets --jq '.[] | select(.name ==
"main") | .id'` gives the ID, and the same pipe goes to `gh api --method PUT repos/{owner}/{repo}/rulesets/<id>`.
When a job of `ci.yml` is renamed or added, change `required_status_checks` in the same PR.
