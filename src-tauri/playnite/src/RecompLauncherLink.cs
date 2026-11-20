// RecompLauncher Link: the small Playnite add-on behind RecompLauncher's "Add to Playnite".
//
// Playnite has no command line or file format for adding a game from outside, so RecompLauncher
// puts this add-on in Playnite's Extensions folder and hands it games through a folder:
//
//   <Playnite's ExtensionsData>\<this add-on's Id>\queue\<key>.json   one game to add or update
//   ...\queue\<key>.icon.png / .cover.jpg / .background.jpg            its pictures, next to it
//
// The add-on takes what's in the queue when Playnite starts, and watches the folder while it runs,
// so a game added from the launcher shows up in Playnite within a second. Each request file is
// deleted once the game is in. A game added again (new art, new name, a new way to start it) is
// updated in place: the add-on finds it by GameId "recomplauncher:<id>", never by name.
//
// While it runs, the add-on keeps link.json in its data folder with Playnite's process id, so the
// launcher can tell whether the Playnite that's open has the add-on loaded (or needs a restart).
using Playnite.SDK;
using Playnite.SDK.Data;
using Playnite.SDK.Events;
using Playnite.SDK.Models;
using Playnite.SDK.Plugins;
using System;
using System.Collections.ObjectModel;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Threading;
using Timer = System.Threading.Timer;

namespace RecompLauncherLink
{
    public class Request
    {
        // The request format. A newer launcher can add fields; the add-on reads what it knows.
        [SerializationPropertyName("v")] public int V { get; set; }
        // "recomplauncher:<game id>": how the add-on finds the game again.
        [SerializationPropertyName("key")] public string Key { get; set; }
        [SerializationPropertyName("name")] public string Name { get; set; }
        // The program Playnite starts, its arguments and the folder it starts in.
        [SerializationPropertyName("path")] public string Path { get; set; }
        [SerializationPropertyName("args")] public string Args { get; set; }
        [SerializationPropertyName("workdir")] public string WorkDir { get; set; }
        // The game's folder (Playnite's Install Directory).
        [SerializationPropertyName("install_dir")] public string InstallDir { get; set; }
        // "directory": Playnite counts play time while any program from tracking_path runs. Used
        // when Playnite starts the game through RecompLauncher, whose own process may hand the game
        // to a launcher that's already open and quit straight away.
        [SerializationPropertyName("tracking")] public string Tracking { get; set; }
        [SerializationPropertyName("tracking_path")] public string TrackingPath { get; set; }
        // Picture file names in the queue folder (empty when there's none).
        [SerializationPropertyName("icon")] public string Icon { get; set; }
        [SerializationPropertyName("cover")] public string Cover { get; set; }
        [SerializationPropertyName("background")] public string Background { get; set; }
    }

    public class Link
    {
        [SerializationPropertyName("version")] public string Version { get; set; }
        [SerializationPropertyName("pid")] public int Pid { get; set; }
    }

    public class RecompLauncherLink : GenericPlugin
    {
        private static readonly ILogger log = LogManager.GetLogger();
        private const string Source = "RecompLauncher";
        private readonly object busy = new object();
        private FileSystemWatcher watcher;
        private Timer later;

        public override Guid Id { get; } = Guid.Parse("458c5c75-a027-41d4-a666-b72ad5236af0");

        public RecompLauncherLink(IPlayniteAPI api) : base(api)
        {
            Properties = new GenericPluginProperties { HasSettings = false };
        }

        private string Queue => System.IO.Path.Combine(GetPluginUserDataPath(), "queue");

        public override void OnApplicationStarted(OnApplicationStartedEventArgs args)
        {
            try
            {
                Directory.CreateDirectory(Queue);
                var link = new Link { Version = GetType().Assembly.GetName().Version.ToString(), Pid = Process.GetCurrentProcess().Id };
                File.WriteAllText(System.IO.Path.Combine(GetPluginUserDataPath(), "link.json"), Serialization.ToJson(link));
                // One import a moment after the last change: the launcher writes pictures first, then
                // the request, and a watcher can fire more than once for one file.
                later = new Timer(_ => ImportAll(), null, Timeout.Infinite, Timeout.Infinite);
                watcher = new FileSystemWatcher(Queue, "*.json") { NotifyFilter = NotifyFilters.FileName | NotifyFilters.LastWrite };
                watcher.Created += (s, e) => Soon();
                watcher.Renamed += (s, e) => Soon();
                watcher.Changed += (s, e) => Soon();
                watcher.EnableRaisingEvents = true;
            }
            catch (Exception e)
            {
                log.Error(e, "RecompLauncher Link couldn't watch its queue");
            }
            ImportAll();
        }

        public override void OnApplicationStopped(OnApplicationStoppedEventArgs args)
        {
            watcher?.Dispose();
            later?.Dispose();
            try { File.Delete(System.IO.Path.Combine(GetPluginUserDataPath(), "link.json")); } catch { }
        }

        private void Soon() => later?.Change(400, Timeout.Infinite);

        // On Playnite's own UI thread: its library view is bound to the database's collections.
        private void ImportAll()
        {
            var ui = PlayniteApi.MainView?.UIDispatcher;
            if (ui != null && !ui.CheckAccess())
            {
                ui.Invoke(new Action(ImportAll));
                return;
            }
            lock (busy)
            {
                string[] files;
                try { files = Directory.GetFiles(Queue, "*.json"); } catch { return; }
                foreach (var file in files)
                {
                    try
                    {
                        Import(file);
                    }
                    catch (Exception e)
                    {
                        log.Error(e, "RecompLauncher Link couldn't add " + System.IO.Path.GetFileName(file));
                        PlayniteApi.Notifications.Add("recomplauncher-link-" + System.IO.Path.GetFileName(file), "RecompLauncher couldn't add a game: " + e.Message, NotificationType.Error);
                    }
                }
            }
        }

        private void Import(string file)
        {
            var r = Serialization.FromJsonFile<Request>(file);
            if (r == null || string.IsNullOrWhiteSpace(r.Key) || string.IsNullOrWhiteSpace(r.Path))
            {
                File.Delete(file);
                return;
            }
            var db = PlayniteApi.Database;
            var game = db.Games.FirstOrDefault(g => g.PluginId == Guid.Empty && g.GameId == r.Key);
            var isNew = game == null;
            if (isNew)
            {
                game = new Game(string.IsNullOrWhiteSpace(r.Name) ? r.Key : r.Name) { GameId = r.Key, Added = DateTime.Now };
            }
            else if (!string.IsNullOrWhiteSpace(r.Name))
            {
                game.Name = r.Name;
            }
            game.IsInstalled = true;
            game.InstallDirectory = r.InstallDir ?? "";
            var source = db.Sources.FirstOrDefault(s => s.Name == Source) ?? db.Sources.Add(Source);
            game.SourceId = source.Id;
            var directory = string.Equals(r.Tracking, "directory", StringComparison.OrdinalIgnoreCase);
            var play = new GameAction
            {
                Name = "Play",
                Type = GameActionType.File,
                IsPlayAction = true,
                Path = r.Path,
                Arguments = r.Args ?? "",
                WorkingDir = r.WorkDir ?? "",
                TrackingMode = directory ? TrackingMode.Directory : TrackingMode.Default,
                TrackingPath = directory ? (r.TrackingPath ?? r.InstallDir ?? "") : "",
            };
            // The add-on's own Play action is replaced; any others the player added stay.
            var others = (game.GameActions ?? new ObservableCollection<GameAction>()).Where(a => !(a.IsPlayAction && a.Name == "Play")).ToList();
            game.GameActions = new ObservableCollection<GameAction>(new[] { play }.Concat(others));
            if (isNew)
            {
                db.Games.Add(game);
            }
            // Pictures go into Playnite's own files for the game (AddFile copies them in).
            var dir = System.IO.Path.GetDirectoryName(file);
            game.Icon = Picture(dir, r.Icon, game.Id, game.Icon);
            game.CoverImage = Picture(dir, r.Cover, game.Id, game.CoverImage);
            game.BackgroundImage = Picture(dir, r.Background, game.Id, game.BackgroundImage);
            db.Games.Update(game);
            foreach (var f in new[] { r.Icon, r.Cover, r.Background }.Where(f => !string.IsNullOrEmpty(f)))
            {
                try { File.Delete(System.IO.Path.Combine(dir, System.IO.Path.GetFileName(f))); } catch { }
            }
            File.Delete(file);
            log.Info("RecompLauncher Link: " + (isNew ? "added " : "updated ") + game.Name);
        }

        private string Picture(string dir, string name, Guid gameId, string old)
        {
            if (string.IsNullOrEmpty(name)) return old;
            var path = System.IO.Path.Combine(dir, System.IO.Path.GetFileName(name));
            if (!File.Exists(path)) return old;
            var added = PlayniteApi.Database.AddFile(path, gameId);
            if (string.IsNullOrEmpty(added)) return old;
            if (!string.IsNullOrEmpty(old) && old != added)
            {
                try { PlayniteApi.Database.RemoveFile(old); } catch { }
            }
            return added;
        }
    }
}
