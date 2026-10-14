# nvmise

> [!IMPORTANT]
> **nvmise has no release yet.** Everything described here is built and
> tested, but there's no download and no Homebrew formula, so for now you
> build it yourself. Few people have used it. Keep `nvmise self uninstall`
> in mind as the way back.
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

This is nvmise 0.1.0, not released yet. It's built on nvmimic 0.1.1 and
tested on macOS, in four shells: bash (including the old bash 3.2 that
macOS ships), zsh, sh and dash.

How close it is to nvm gets measured. nvmimic has a test harness that runs
official nvm and another program side by side and compares everything they
print, return, set and write. nvmise goes through the same harness. What
differs is listed under [What's different from nvm](#whats-different-from-nvm).

## Installing

You need mise. There's no download of nvmise yet, so build it with
[Rust](https://www.rust-lang.org):

```sh
cargo build --release
./target/release/nvmise self install
```

Once there's a release, [Homebrew](https://brew.sh) will be the short way,
from the same tap as the author's other tools. This doesn't work yet:

```sh
brew install tsilenzio/tap/nvmise
nvmise self install
```

`nvmise self install` sets nvmise up for `$NVM_DIR` (usually `~/.nvm`). It
shows what it's about to change and asks first. You don't need nvm on the
machine. If nvm is there, this is what happens:

- The three scripts other tools look for in the nvm folder (`nvm.sh`,
  `nvm-exec` and `bash_completion`) become nvmise's. Official nvm's are kept
  in a backup folder, so they can be put back.
- Nothing else in the nvm folder changes. The Node versions and aliases
  official nvm kept there stay where they are. nvmise doesn't use them: its
  Node versions are mise's, and its aliases are its own.
- It offers to copy the whole nvm folder somewhere safe first.
- If you installed nvm with Homebrew, it asks before running `brew
  uninstall nvm`, because a shell that loads Homebrew's copy would keep
  running official nvm.

`self install` also copies the nvmise program to `~/.local/bin/nvmise`,
unless Homebrew or cargo installed it. It never puts a program named `nvm`
on your `PATH`, because official nvm is a shell function and scripts check
for that.

Every `self` command takes two flags. `--dry-run` shows what would happen
and changes nothing. `--force` goes ahead without asking. Where there's no
terminal to ask at, such as inside a script, a command changes nothing
unless you pass `--force`. `--help` on any command prints a short reminder.

### Loading nvm in your shell

A shell gets the `nvm` command from a line in its startup file, such as
`~/.zshrc` or `~/.bashrc`. If nvm was on the machine before, the line is
already there and you change nothing:

```sh
export NVM_DIR="$HOME/.nvm"
[ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh"  # This loads nvm
```

Tools that look for `$NVM_DIR/nvm.sh` by themselves find nvmise the same
way.

You can also ask the nvmise program directly, without `self install`. Give
it the name of the shell: `bash`, `zsh`, `sh` or `dash`.

```sh
eval "$(nvmise init zsh)"
```

That loads `nvm` into the one shell that runs the line. Tools that load
`$NVM_DIR/nvm.sh` themselves won't see it.

### Removing nvmise

`nvmise self uninstall` removes nvmise. It shows what it's about to do and
asks first.

- `nvm` goes to nvmimic, if nvmimic is set up and wasn't switched off.
  Otherwise official nvm's three scripts go back into the nvm folder.
- The aliases you made with nvmise are deleted. They're listed before you
  say yes.
- The Node versions stay. They're mise's, and `mise ls node` still shows
  them. The nvm folder stays too.

If there was no nvm before nvmise and nvmimic isn't there, you're left
with no `nvm` command. nvmise doesn't carry a copy of official nvm.

If Homebrew or cargo installed nvmise, `self uninstall` leaves the program
alone and tells you the command that removes it. Run `nvmise self
uninstall` before `brew uninstall nvmise`.

### Updating nvmise

`nvmise self update` will get the newest nvmise, check the download and
swap it in, and `nvmise self rollback` will bring the old one back. Both
are built, but there's no release to update to yet. A copy you built
yourself can't check who signed a download, so it refuses to update
itself. Build the newer version the same way you built this one.

## Switching nvmise off and on

`nvmise self disable` hands the `nvm` command to someone else and keeps
nvmise installed, with its aliases. If nvmimic is set up on the machine,
nvmimic takes over. If it isn't, official nvm's scripts go back.

`nvmise self enable` takes `nvm` back.

Shells that are already open keep the `nvm` they loaded. A new shell gets
whoever answers now.

### The sister tool, nvmimic

[nvmimic](https://github.com/tsilenzio/nvmimic) and nvmise are sister
tools by the same author, built on the same library. nvmimic is nvm, only
faster: it keeps Node in nvm's own folders and doesn't need mise. nvmise
keeps Node in mise.

You can have both installed. Only one of them answers `nvm` at a time, and
whichever you last told to take over is the one:

- `nvmise self install` or `nvmise self enable` while nvmimic answers:
  nvmise says what will change, names nvmimic, and asks before it takes
  over. nvmimic stays installed and asleep.
- `nvmimic self install` while nvmise answers: the same, the other way
  round.
- `nvmise self uninstall` while nvmise answers: `nvm` goes to nvmimic.
- `nvmise self uninstall` while nvmimic answers: nvmise goes, and nothing
  else changes.

One thing to know: a tool that was switched off isn't handed `nvm` behind
your back. If you ran `nvmise self disable` and later uninstall nvmimic,
official nvm goes back, and nvmise stays asleep until `nvmise self enable`.

The two keep one small folder together, `~/.local/state/nvmimic-shared`.
It holds official nvm's three scripts and a note of who answers `nvm`.
Don't delete it while either tool is installed. The last one to be
uninstalled removes it.

nvmimic 0.1.1 or newer is needed for this. An older nvmimic doesn't know
the shared folder, and nvmise refuses to take over from it.

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
with mise switched on, nvmise leaves the new shell exactly as it is: `PATH`
isn't touched, and mise picks the Node version, from the folder's
`mise.toml` or `.nvmrc` or from your mise settings. The `default` alias
only matters there once you type `nvm use default`.

For this to work, the line that switches mise on has to come before the
line that loads nvm in your startup file. nvmise can only see a mise that
is already there.

**nvmise only tells mise about versions that are installed.** With mise's
stand-ins for `node` on your `PATH`, asking for a version that isn't there
would make mise install it without a word.

## What's different from nvm

- **No building from source.** `nvm install -s` is turned down with a
  message. mise unpacks ready-made releases.
- **No nvm download cache.** mise does the downloading and keeps its own
  cache. `nvm cache dir`, `nvm cache clear` and `nvm install --offline` are
  turned down with a message.
- **Other lines while installing.** nvm prints `Downloading https://...`
  and `Checksums matched!`. With nvmise you see mise's own progress lines
  there. `Downloading and installing node v22.1.0...` and `Now using node
  v22.1.0 (npm v10.7.0)` are the same.
- **No io.js.** mise keeps Node only.
- **No `current` link.** `NVM_SYMLINK_CURRENT` makes no link in the nvm
  folder.
- **No install hook.** `NVM_INSTALL_THIRD_PARTY_HOOK` isn't run, because
  mise wouldn't know about a version something else installed.
- **Nothing in the nvm folder.** A script that looks into
  `$NVM_DIR/versions` or `$NVM_DIR/alias` by itself finds no Node versions
  and no aliases there. `nvm which` and `nvm alias` are the way to ask.
- **`mise prune` can delete what `nvm install` installed.** mise's `prune`
  removes versions that no mise config asks for, and a version you
  installed with `nvm install` is one of those.
- **`nvm ls` may show a `system` line** in a shell with mise switched on.
  That's mise's stand-in for `node`, which nvm takes for a Node of the
  system.
- **npm is mise's.** mise puts a small script of its own in place of
  `bin/npm`. After `npm install -g` it calls `mise`, so mise has to be on
  your `PATH` for that, even when `NVMISE_MISE` says where it is.

Tab completion leaves out what nvmise can't do.

## Where nvmise keeps its files

In the nvm folder, nvmise only swaps the three scripts. Everything else of
its own goes where many command line tools keep theirs. Each place below
has a variable that moves it. Set it before you run `self install`.

| What | Where | Variable |
|---|---|---|
| Node versions | mise's folder, `~/.local/share/mise/installs/node` | mise's own `MISE_DATA_DIR` |
| Aliases, such as `default` | `~/.local/share/nvmise/alias` | `NVMISE_DATA_DIR` |
| The nvmise program | `~/.local/bin/nvmise` | `NVMISE_INSTALL_PATH` |
| The list of what was installed | `~/.local/state/nvmise` | `NVMISE_STATE_DIR` |
| Official nvm's scripts and who answers `nvm`, shared with nvmimic | `~/.local/state/nvmimic-shared` | `NVMIMIC_SHARED_DIR` |
| Install locks and remembered answers, safe to delete | `~/Library/Caches/nvmise` on macOS, `~/.cache/nvmise` elsewhere | `NVMISE_CACHE_DIR` |

nvmise looks for the `mise` program on your `PATH`. Set `NVMISE_MISE` to
the full path of the program if it's somewhere else.

## Working on it

- `cargo test` runs the tests. They use throwaway home folders and a small
  script standing in for mise, so they never touch your Node versions, your
  nvm folder or your mise settings. If nvmimic 0.1.1 or newer is installed
  with Homebrew, `tests/sister.rs` also runs both tools in one nvm folder.
  Without it, those tests skip themselves.
- `cargo test --test real_mise -- --ignored --nocapture` is a smoke test
  with the mise program of your machine. It still runs in a throwaway home,
  with every folder mise uses pointed inside it. It downloads one Node
  release from nodejs.org the first time and keeps it under `target/`. Then
  it installs, uses, lists and uninstalls that version through `nvm` in
  bash, zsh, sh and dash, with mise switched on and off, and prints what
  mise asked for and said.
- `cargo test --test parity -- --ignored --nocapture` compares nvmise with
  official nvm through nvmimic's harness. The first run records what
  official nvm does for every test script, which takes a while. Run it
  alone. The differences that are there on purpose are listed in
  `tests/parity/expected`.

The code is small, because the library does the heavy part:

- `src/backend.rs` is the backend for mise.
- `src/identity.rs` is the name nvmise goes by. The library works folder
  names, setting names and shell function names out from it.
- `src/setup.rs` is nvmise's own part of `self install` and `self
  uninstall`: its folder of aliases.
- `src/main.rs` is the program: the `self` commands, `init`, and the
  answer to the shell functions.

## License

MIT, the same license as nvm and nvmimic. nvmise is built on nvmimic, and
parts of nvmimic are ported from nvm, so nvm's copyright notice sits next
to ours in [LICENSE](LICENSE).
