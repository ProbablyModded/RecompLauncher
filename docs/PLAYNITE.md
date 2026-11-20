# Add to Playnite

**Add To → Add to Playnite** on an installed game (the "…" menu on its page or its card, and the
options sheet in Big Picture mode) puts the game in [Playnite](https://playnite.link)'s library,
with its cover, background and icon. It shows on Windows when Playnite is on the PC.

## How the game starts

The same way the game's desktop shortcut does, following **Settings → Desktop shortcuts → Shortcuts
Go Through the Launcher**:

- **Off** (the default): Playnite starts the game's own program, with its arguments, straight from
  the game's folder. Playnite counts the play time itself.
- **On**: Playnite starts `RecompLauncher --play <id>`, so your shader, cloud saves, backups and
  the launcher's play time come along. Playnite tracks the game by its folder (any program running
  from it), because the launcher's own process can hand the game to a launcher that's already open
  and quit straight away.

A game that only the launcher can start for now (a setup still to run, a copy still to pick, one
game of several in a recomp) always goes through the launcher, as its desktop shortcut does.

Pressing Add to Playnite again updates the same Playnite entry (name, art, how it starts). Anything
else you changed on it in Playnite stays, and so do other Play actions you added.

## How it gets into Playnite

Playnite has no command line or file format for adding a game from another program, and its
library is a database only Playnite should write. Playnite's own way in is an add-on, so the
launcher brings a small one, **RecompLauncher Link**:

1. The first time, the launcher copies the add-on into Playnite's `Extensions` folder
   (`%AppData%\Playnite\Extensions\RecompLauncher_Link_458c5c75`, or the `Extensions` folder of a
   portable Playnite). Playnite loads add-ons when it starts.
2. Each game goes to the add-on as a request file, with its pictures beside it, in the add-on's own
   data folder (`ExtensionsData\458c5c75-a027-41d4-a666-b72ad5236af0\queue`).
3. The add-on adds what's waiting when Playnite starts, and watches the folder while Playnite is
   open, so the game appears within a second. It deletes each request once the game is in.

If Playnite is open but started before the add-on was there, the launcher asks to restart Playnite
(as Add to Steam does with Steam). Say Not Now and the game goes in the next time Playnite starts.
If Playnite is closed, the game goes in the next time it opens.

Games added this way have **RecompLauncher** as their Source in Playnite, and a GameId of
`recomplauncher:<id>`, which is how the add-on finds them again.

## The add-on

Source: `src-tauri/playnite/src/RecompLauncherLink.cs` (about 200 lines, a Playnite
`GenericPlugin` for Playnite 9 and 10, SDK 6.4). It reads only its own queue folder, and changes
only the games it added. It has no settings and makes no network requests.

The built DLL is committed at `src-tauri/playnite/RecompLauncherLink.dll` so a plain
`tauri build` has it, and the launcher carries it with `include_bytes!`. CI and the fork's release
workflow build it again from source first (`tools/build-playnite-link.sh`, needs the .NET SDK).
The build is deterministic: the same source and SDK give the same bytes.

To remove it: Playnite → Add-ons → Extensions → RecompLauncher Link → Uninstall. The games it
added stay in Playnite.
