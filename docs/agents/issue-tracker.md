# Issue tracker: GitHub

Issues and specs live in GitHub Issues for `evgen2571/vestra`.
Use the `gh` CLI from this checkout.

## Development roadmap

The [Vestra development roadmap](https://github.com/evgen2571/vestra/issues/63)
links the workstream specs and prioritized delivery tickets. Before starting a
ticket, read its owning spec, agent brief and native blocking relationships.
Accepted research/prototype results require a separately reviewed implementation
ticket before production adoption.

## Operations

- Create: `gh issue create --title "..." --body-file <file>`.
- Read: `gh issue view <number> --json number,title,body,labels,comments`.
- List: `gh issue list --state open`, with appropriate label filters.
- Comment: `gh issue comment <number> --body-file <file>`.
- Label: `gh issue edit <number> --add-label "..." --remove-label "..."`.
- Close: `gh issue close <number> --comment "..."`.

For multiline bodies, write the exact Markdown to a temporary file and use
`--body-file`.

## Pull requests as a triage surface

**PRs as a request surface: no.**

GitHub shares issue and PR numbers. Resolve an ambiguous number with
`gh pr view <number>`, falling back to `gh issue view <number>`.

## Skill instructions

“Publish to the issue tracker” means create a GitHub issue.
“Fetch the relevant ticket” means read its body, labels and comments.

## Wayfinding operations

- Map: one issue labelled `wayfinder:map`, containing Notes,
  Decisions-so-far and Fog.
- Child tickets: link as native GitHub sub-issues where available.
  Otherwise add a task-list entry in the map and `Part of #<map>`
  at the top of the child body.
- Ticket types: `wayfinder:research`, `wayfinder:prototype`,
  `wayfinder:grilling`, or `wayfinder:task`.
- Blocking: use native GitHub issue dependencies where available.
  Dependency API calls require numeric database IDs, not issue numbers.
  Otherwise record `Blocked by: #<number>` at the top of the child body.
- Frontier: choose the first open, unassigned child in map order
  whose blockers are all closed.
- Claim: `gh issue edit <number> --add-assignee @me`.
- Resolve: comment with the result, close the ticket, and add a brief
  result with a link to the map's Decisions-so-far.
