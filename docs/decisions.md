---
title: Decisions
description: "The decisions currently in force, each with its reason and consequence."
order: 5
---

Decisions currently in force. When one is superseded, this file describes the new
state — it is not a history.

> **Scripts are the product, not one feature among several**
>
> The start screen shows the project's scripts immediately — grouped and
> labelled, with no area selection in front of them. Health, updates, clean and
> security are reachable by hotkey (`H`, `U`, `C`, `S`), never by navigating
> through a menu layer first.
>
> *Reason:* the shortest path to running `dev` is what the tool is used for
> every day. An earlier draft put a "Run / Quality / Maintenance" selection in
> front of the scripts; that makes the common case slower to serve the rare one.
>
> *Consequence:* `Enter` on the highlighted entry runs it. Anything that adds a
> step between opening `opi` and starting a script needs to justify itself
> against this.

> **A long list pages by group rather than scrolling past every one**
>
> Past 15 entries or 5 groups the groups become a row of tabs and only the
> active one is listed. The first group is preselected with the cursor on its
> first entry, so `opi ↵` runs the same script it did before.
>
> *Reason:* the flat screen was decided against 24 entries in 8 groups, which
> still read at a glance. At 27 entries in 7 groups — one real project's root —
> a heading per group makes 34 lines, so the top scrolls away and the screen
> stops being a screen. The rejected submenu demanded a choice before anything
> runnable was visible; a tab row demands nothing and hides what is not needed.
>
> *Consequence:* the threshold is a count, not the terminal's height, which
> would have fitted more exactly. The height changes while the menu is open,
> and a list that rearranged itself mid-keystroke would move entries under a
> cursor already on its way to one. `opi` therefore looks the same in every
> window, and `opi | cat` lists every group as before — a pipe cannot press a
> key to reach the second tab.
>
> *Consequence:* `/` keeps searching every group. Tabs answer "I know roughly
> where"; the filter answers "I know exactly what", and a query matching four
> groups is precisely the case tabs are worst at.
>
> *Consequence:* every tab names an action, except one that names the
> workspace's packages and is marked off by a divider. One tab per package was
> built first and measured wrong: 52 packages against 6 action groups is a row
> of 58, and the two kinds stood side by side as peers, so "where is the thing
> that deploys insights?" had two right answers. Grouping is unchanged — each
> package keeps its heading inside the shared tab.

> **The script name is not the user interface**
>
> Groups are derived from script name prefixes — the part before the first `:`
> becomes the group, known base names (`dev`, `check`, `test`) map to meaningful
> labels. Group order is fixed and meaning-oriented, not alphabetical.
> Descriptions come from `opi.scripts.<name>.description`, then `scripts-info`;
> where neither exists, the column stays empty.
>
> *Reason:* a flat alphabetical list of raw script names is the problem `opi`
> exists to solve.
>
> *Consequence:* the description column is never filled with the raw command. A
> command line shown as a "description" reintroduces exactly the technical
> surface being removed.

> **runemark is extended rather than paired with a second library**
>
> The interactive list lives in runemark, behind its `select` feature, and so
> do its viewport, its width fitting and its filter. `opi` takes on no other
> presentation dependency — no ratatui, no inquire.
>
> *Reason:* one presentation layer across the CLI tools in this ecosystem,
> rather than two competing rendering models inside one binary.
>
> *Consequence:* runemark's design boundary now covers grouped selection, and
> its documentation says so. The API stays generic — it knows nothing about
> scripts, `package.json` or package managers. Four rounds of `opi`'s needs
> shaped it without a single `opi` term entering it. If that generic form ever
> turns out not to carry the interface, that is grounds to revisit this
> decision, not to widen the boundary further.

> **Unix only**
>
> `opi` does not build on Windows, and says so at compile time.
>
> *Reason:* running a script by replacing the process with `exec`, and driving
> termios for the interactive list, have no Windows equivalent. Supporting it
> would mean a second execution model with different `Ctrl-C` semantics and no
> coverage.
>
> *Consequence:* runemark's `select` feature is Unix-only for the same reason,
> while the rest of runemark stays cross-platform for its other consumers.

> **The crate is named `opi`, with a known collision**
>
> Verified against the crates.io API on 2026-09-18: `opi` itself is unclaimed,
> but the `opi-*` namespace — `opi-agent`, `opi-ai`, `opi-tui`, `opi-sandbox`,
> `opi-protocol`, `opi-coding-agent`, all at `0.8.1` — belongs to
> [OdradekAI/opi](https://github.com/OdradekAI/opi), an unrelated coding agent
> whose CLI is most likely also called `opi`.
>
> *Reason:* the name states what the tool is, and it is still registrable.
>
> *Consequence:* three things follow. Installing both tools puts two `opi`
> binaries in `~/.cargo/bin`, where one overwrites the other. A registry search
> for "opi" surfaces the other project first. And `opi-core`, `opi-adapters` and
> similar are unavailable, so any later split into multiple crates needs
> differently named internal crates. The name is also not reserved — publishing
> early is what secures it.

> **A favourite leaves its group**
>
> A script marked `favorite` moves into a Favorites group at the top rather
> than sorting first inside the group its name implies.
>
> *Reason:* sorting first within a group barely shows. Measured on a real
> project, five of its ten groups held two entries each.
>
> *Consequence:* a favourite in a workspace member is the exception — it stays
> with its member, because lifting it out would lose which package it belongs
> to and its id would no longer say.

> **More than one ecosystem, and none of them wins**
>
> `package.json` and `Cargo.toml` are both looked for, and a repository that is
> both shows both under one set of headings.
>
> *Reason:* measured across 231 directories here, 133 carry a `package.json`,
> 51 a `Cargo.toml` and twelve both. Choosing one would misrepresent those
> twelve, and refusing a repository that has only the second turned 39 of them
> away.
>
> *Consequence:* `Task` carries how to start it — through the package manager,
> or by naming its own program. A third ecosystem needs no second list.

> **Built-in actions never shadow a project's scripts**
>
> A project with a script named `health`, `clean`, `release` or `commit` is not
> merely plausible — `clean` appeared in 41 of 133 measured projects. Where a
> name collides the script wins, and every built-in area is reached by a flag
> or a hotkey instead of a bare word.
>
> *Reason:* `opi` must not break in exactly the kind of project it is meant to
> improve.

> **Reporting beats acting, where acting is a guess**
>
> `opi --updates` separates what is outdated by semver jump, and applies only
> the safe ones, only on request, and only by calling the package manager with
> a list of names. Majors never ride along. Clean removes only what it has
> measured and been told to; `opi --hooks` appends one line to a hook.
>
> *Reason:* the package manager rewrites `package.json`, the lockfile and pnpm
> catalogs correctly — measured, including the 45 of 79 workspaces here that
> use a catalog. opi writing any of them itself would be the guess. (The rule
> used to be "updates are never applied"; see
> [constraints.md](constraints.md) for why it narrowed.)
>
> *Consequence:* the value of the screen is the separation — "two safe, one
> major" is a decision, a column of version numbers is homework.

> **A `Report` where the data is parsed, raw output where it is not**
>
> The dependency audit and the update list render as runemark `Report`s. Health,
> the workflows and the secret scan print the tool's own output instead.
>
> *Reason:* a `Finding` is a message. Relayed output is not one — putting it in
> a single finding collapses its structure, and splitting it per line flattens
> the tool's indentation and counts lines as findings. Both were tried against
> real output and both read worse than the tool itself.
>
> *Consequence:* `opi` uses roughly half of runemark's surface, and that is the
> intended answer rather than a gap. The half it does not use is for
> applications that own their finding types; `opi` deliberately does not.

> **No health score**
>
> Health reports concrete results — counts, findings, file locations — and never
> a synthetic figure like "87/100".
>
> *Reason:* a composite score stops meaning anything within weeks; concrete
> results stay useful.

> **opi is the CI for repositories without one**
>
> `opi --check push` runs every check and the build, and `opi --hooks` installs
> it as the pre-push hook. A failing run blocks the push; `git push --no-verify`
> is the way past it.
>
> *Reason:* the private repositories here run no CI on push, and the
> replacement was a hand-written `verify` script and husky hook per repository.
> opi already knew the tools; what was missing was the build, a gate on the
> upstream branch and a hook nobody has to maintain.
>
> *Consequence:* workflows are no longer only "a convenience before pushing".
> The push workflow is meant to be binding, which is why it blocks rather than
> warns, and why its gates stay offline — a hook that waits on the network is a
> hook people disable.
