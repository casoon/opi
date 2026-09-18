# Decisions

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

> **The script name is not the user interface**
>
> Groups are derived from script name prefixes — the part before the first `:`
> becomes the group, known base names (`dev`, `check`, `test`) map to meaningful
> labels. Group order is fixed and meaning-oriented, not alphabetical.
> Descriptions come from `scripts-info`; where absent, the column stays empty.
>
> *Reason:* a flat alphabetical list of raw script names is the problem `opi`
> exists to solve.
>
> *Consequence:* the description column is never filled with the raw command. A
> command line shown as a "description" reintroduces exactly the technical
> surface being removed.

> **runemark is extended rather than paired with a second library**
>
> The interactive selection layer lives in runemark `0.4.0`, behind its `select`
> feature. `opi` takes on no other presentation dependency — no ratatui, no
> inquire.
>
> *Reason:* one presentation layer across the CLI tools in this ecosystem,
> rather than two competing rendering models inside one binary.
>
> *Consequence:* runemark's design boundary now covers grouped selection, and
> its documentation says so. The API stays generic — it knows nothing about
> scripts, `package.json` or package managers. If that generic form turns out
> not to carry `opi`'s interface, that is grounds to revisit this decision, not
> to widen the boundary further.

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

> **Built-in actions never shadow a project's scripts**
>
> A project with a script named `health`, `clean` or `update` is plausible.
> Where a name collides, the script wins; built-in actions stay reachable by
> hotkey and in an unambiguous form.
>
> *Reason:* `opi` must not break in exactly the kind of project it is meant to
> improve.

> **No health score**
>
> Health reports concrete results — counts, findings, file locations — and never
> a synthetic figure like "87/100".
>
> *Reason:* a composite score stops meaning anything within weeks; concrete
> results stay useful.
