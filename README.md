# nvmise

> [!IMPORTANT]
> **nvmise isn't finished.** You can build it and load it into a shell by
> hand today. The commands that set it up for you (`self install`, `self
> enable`, `self disable`, `self uninstall`) aren't built yet. The sections
> about them below say so.
>
> nvmise isn't affiliated with nvm-sh, the nvm project, the OpenJS
> Foundation or mise.

nvmise gives you the `nvm` command, with [mise](https://mise.jdx.dev) doing
the work behind it. [nvm](https://github.com/nvm-sh/nvm) is the tool many
people use to install Node and switch between Node versions. mise is a newer
tool that manages Node and many other tools. With nvmise, a script that
calls `nvm install` or `nvm use` keeps working, and the Node versions it
installs and switches to are the ones mise keeps.

## Who it's for

You use mise, and you'd like it to be the only thing that installs Node on
your machine. But something you can't change still expects nvm. It might be
a setup script that loads `$NVM_DIR/nvm.sh` and then runs `nvm install`, or
an editor plugin that calls `nvm use`.

Without nvmise you end up with two sets of Node versions, one in nvm's
folder and one in mise's, and two tools that each think they picked the
active one. With nvmise there is one set, in mise's folder.

If nothing on your machine needs the `nvm` command, you don't need nvmise.
Use mise by itself. If you want nvm itself, only faster, look at
[nvmimic](#the-sister-tool-nvmimic).

## How it works

nvmise doesn't copy nvm's behavior itself. That part comes from the library
inside [nvmimic](https://github.com/tsilenzio/nvmimic), a copy of nvm 0.40.8
written in Rust. The library knows every nvm command, what each one prints
and which exit code it returns. It reads `.nvmrc` files, understands names
such as `lts/*` and `node`, and keeps aliases such as `default`.

The one thing the library doesn't decide is where Node versions live. It
asks a backend for that. nvmise is a backend for mise, plus a small program
around it:

- **Listing and finding versions** only reads mise's folder of Node
  versions (`~/.local/share/mise/installs/node`). No other program is
  started, so `nvm ls`, `nvm which` and `nvm current` are quick.
- **`nvm install`** works out the exact version first, the way nvm does,
  and then runs `mise install node@<that version>`. mise downloads the
  release, checks it and unpacks it.
- **`nvm uninstall`** runs `mise uninstall node@<that version>`.
- **`nvm use`** changes `PATH` in your shell, as nvm does. A program can't
  change the shell that started it, so `nvm` is a small shell function that
  starts the nvmise program and then does what the program prints.
- **Aliases** are nvm's idea and mise has nothing like them. nvmise keeps
  them in a folder of its own.

Because the nvm side comes from that library, what you see is nvm's output:
`Downloading and installing node v22.1.0...`, `Now using node v22.1.0 (npm
v10.7.0)`, `v22.1.0 is already installed.`, `N/A` for a version that isn't
there. One thing differs on purpose: paths point into mise's folder, so `nvm
which 22` prints something like
`~/.local/share/mise/installs/node/22.1.0/bin/node`.

## Status

This is nvmise 0.1.0, not released. It's built on nvmimic 0.1.0 and tested
on macOS.

Built:

- The `nvm` commands, through the library, with Node kept in mise's folder.
- `nvmise init`, which prints the shell code that loads `nvm`.
- Four shells: bash (including the old bash 3.2 that macOS ships), zsh, sh
  and dash.

Not built yet:

- `nvmise self install`, `self enable`, `self disable`, `self uninstall`
  and `self update`.
- A download or a Homebrew formula.

## Trying it today

You need [Rust](https://www.rust-lang.org) and mise. Build the program:

```sh
cargo build --release
```

Then load it in a shell. Give `init` the name of that shell: `bash`, `zsh`,
`sh` or `dash`.

```sh
export NVM_DIR="$HOME/.nvm"
eval "$(./target/release/nvmise init zsh)"
```

That shell now has an `nvm` that keeps Node in mise's folder. Nothing on
disk was changed, and a new shell knows nothing about it. `nvm unload`
takes it out of the shell again.

A tool that loads `$NVM_DIR/nvm.sh` by itself won't find nvmise this way.
That needs `self install`, which isn't built yet.

## Installing

> [!NOTE]
> Not built yet. This section describes the plan.

nvmise will come from the same Homebrew tap as the author's other tools:

```sh
brew install tsilenzio/tap/nvmise
nvmise self install
```

Homebrew will install the program only. `nvmise self install` is the step
that sets it up, and you run it yourself. It will show what it's about to
change and ask first. With `--dry-run` it will show the same list and
change nothing.

What it will change: the three scripts other tools look for in your nvm
folder (`nvm.sh`, `nvm-exec` and `bash_completion`) become nvmise's. The nvm
folder itself stays where it is, with everything else in it untouched. The
scripts that were there are kept safe so they can be put back. If you
installed nvm with Homebrew, nvmise will ask before running `brew uninstall
nvm`, because a shell that loads Homebrew's copy would keep running
official nvm.

`nvmise self uninstall` will put things back the way they were and delete
nvmise's own files, its aliases included. It will list them first.

## Switching nvmise off and on

> [!NOTE]
> Not built yet. This section describes the plan.

`nvmise self disable` will hand the `nvm` command to someone else without
removing nvmise. If nvmimic is set up on the machine, nvmimic takes over.
If it isn't, official nvm does. `nvmise self enable` takes `nvm` back.

### The sister tool, nvmimic

[nvmimic](https://github.com/tsilenzio/nvmimic) and nvmise are sister
tools by the same author, built on the same library. nvmimic is nvm, only
faster: it keeps Node in nvm's own folders and doesn't need mise. nvmise
keeps Node in mise.

You'll be able to have both installed. Only one of them answers `nvm` at a
time, and whichever you last told to take over is the one. Their messages
will name the other tool when it's involved.

## nvmise and mise in the same shell

mise can be switched on in a shell with `mise activate`. It then picks a
Node version for the folder you're in and puts it on `PATH` before every
prompt. nvm wants to do the same job. Here is how nvmise keeps the two from
fighting.

**`nvm use 22` sticks.** In a shell without mise switched on, nvmise changes
`PATH` and that's all it takes. In a shell with mise switched on, nvmise
also sets `MISE_NODE_VERSION`, which is mise's own setting for "use this
Node here". mise then agrees with `nvm use` and keeps that version when you
change folders. `nvm deactivate` takes the setting away again.

One thing follows from that. After `nvm use` in a shell with mise switched
on, a folder's own `mise.toml` or `.nvmrc` doesn't change the Node version
in that shell any more, until you run `nvm deactivate`. That's how nvm
behaves too: the version you chose stays until you choose another.

**A new shell never overrides mise.** nvm switches every new shell to your
`default` alias. In a shell without mise, nvmise does the same. In a shell
with mise switched on, mise's pick is meant to win. That works today only
in part. nvmise doesn't tell mise about the switch, so mise puts its own
pick back at the first prompt. But `PATH` is still changed until then, and
in one case (when the shell has to start nvmise a second time while
loading) mise is told after all. Closing that gap needs a change in the
library, which is asked for.

**nvmise only tells mise about versions that are installed.** With mise's
stand-ins for `node` on your `PATH`, asking for a version that isn't there
would make mise install it without a word.

## What's different from nvm

- **No building from source.** `nvm install -s` is turned down with a
  message. mise unpacks ready-made releases.
- **No io.js.** mise keeps Node only.
- **No `current` link.** `NVM_SYMLINK_CURRENT` makes no link in the nvm
  folder.
- **No install hook.** `NVM_INSTALL_THIRD_PARTY_HOOK` isn't run, because
  mise wouldn't know about a version something else installed.
- **`mise prune` can delete what `nvm install` installed.** mise's `prune`
  removes versions that no mise config asks for, and a version you
  installed with `nvm install` is one of those.
- **`nvm ls` may show a `system` line** in a shell with mise switched on.
  That's mise's stand-in for `node`, which nvm takes for a Node of the
  system.

Tab completion leaves out what nvmise can't do.

## Where nvmise keeps its files

Nothing of nvmise's goes into your nvm folder today. Each place below has a
variable that moves it.

| What | Where | Variable |
|---|---|---|
| Node versions | mise's folder, `~/.local/share/mise/installs/node` | mise's own `MISE_DATA_DIR` |
| Aliases, such as `default` | `~/.local/share/nvmise/alias` | `NVMISE_DATA_DIR` |
| Install locks and remembered answers, safe to delete | `~/Library/Caches/nvmise` on macOS, `~/.cache/nvmise` elsewhere | `NVMISE_CACHE_DIR` |

nvmise looks for the `mise` program on your `PATH`. Set `NVMISE_MISE` to
the full path of the program if it's somewhere else.

## Working on it

- `cargo test` runs the tests. They use a throwaway home folder and a small
  script standing in for mise, so they never touch your Node versions, your
  nvm folder or your mise settings.
- `cargo test --test real_mise -- --ignored --nocapture` is a smoke test
  with the mise program of your machine. It still runs in a throwaway home,
  with every folder mise uses pointed inside it. It downloads one Node
  release from nodejs.org the first time and keeps it under `target/`. Then
  it installs, uses, lists and uninstalls that version through `nvm` in
  bash, zsh, sh and dash, with mise switched on and off, and prints what
  mise asked for and said.

The code is small, because the library does the heavy part:

- `src/backend.rs` is the backend for mise.
- `src/identity.rs` is the name nvmise goes by. The library works folder
  names, setting names and shell function names out from it.
- `src/main.rs` is the program: `init`, and the answer to the shell
  functions.

## License

MIT, the same license as nvm and nvmimic. nvmise is built on nvmimic, and
parts of nvmimic are ported from nvm, so nvm's copyright notice sits next
to ours in [LICENSE](LICENSE).
