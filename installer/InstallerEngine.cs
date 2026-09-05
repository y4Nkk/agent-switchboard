using System;
using System.Diagnostics;
using System.IO;
using System.Net;
using System.Reflection;
using System.Threading.Tasks;
using Microsoft.Win32;

namespace AgentSwitchboard.Installer
{
    public sealed class InstallResult
    {
        public int ExitCode { get; set; }
        public string LaunchError { get; set; }
    }

    public static class InstallerEngine
    {
        internal const string PayloadResource = "AgentSwitchboard.Installer.Engine.exe";
        internal static readonly string[] Package = ReadPackage();

        private static string[] ReadPackage()
        {
            using (var stream = Assembly.GetExecutingAssembly().GetManifestResourceStream("AgentSwitchboard.Installer.Package.txt"))
            {
                if (stream == null) throw new InvalidDataException("Installer package metadata is missing.");
                using (var reader = new StreamReader(stream))
                {
                    var values = reader.ReadToEnd().Trim().Split(new[] { '\r', '\n' }, StringSplitOptions.RemoveEmptyEntries);
                    if (values.Length != 3) throw new InvalidDataException("Installer package metadata is invalid.");
                    return values;
                }
            }
        }

        public static string DetectDirectory()
        {
            using (var hive = RegistryKey.OpenBaseKey(RegistryHive.CurrentUser, RegistryView.Registry64))
            using (var key = hive.OpenSubKey("Software\\" + Package[2] + "\\" + Package[0]))
            {
                var installed = key == null ? null : key.GetValue(null) as string;
                if (!String.IsNullOrWhiteSpace(installed)) return installed;
            }
            return Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), Package[0]);
        }

        public static async Task<InstallResult> Run(InstallOptions options)
        {
            string arguments = options.EngineArguments();
            string temporary = Path.Combine(Path.GetTempPath(), "agent-switchboard-setup-" + Guid.NewGuid().ToString("N"));
            System.IO.Directory.CreateDirectory(temporary);
            try
            {
                await EnsureWebView(temporary);
                string engine = Path.Combine(temporary, "engine.exe");
                using (var input = Assembly.GetExecutingAssembly().GetManifestResourceStream(PayloadResource))
                {
                    if (input == null) throw new InvalidDataException("Installer engine is missing.");
                    using (var output = File.Create(engine)) await input.CopyToAsync(output);
                }
                int code = await RunProcess(engine, arguments);
                var result = new InstallResult { ExitCode = code };
                if (code == 0 && options.Restart)
                {
                    try { Launch(options.Directory, options.ApplicationArguments()); }
                    catch (Exception error) { result.LaunchError = error.Message; Trace.TraceWarning("Application restart: " + error.Message); }
                }
                return result;
            }
            finally
            {
                // Only this invocation's generated staging folder is removed.
                try { System.IO.Directory.Delete(temporary, true); }
                catch (IOException error) { Trace.TraceWarning("Installer staging cleanup: " + error.Message); }
                catch (UnauthorizedAccessException error) { Trace.TraceWarning("Installer staging cleanup: " + error.Message); }
            }
        }

        internal static Task<int> RunProcess(string executable, string arguments)
        {
            return Task.Run(() =>
            {
                using (var process = Process.Start(new ProcessStartInfo(executable, arguments) {
                    UseShellExecute = false, CreateNoWindow = true,
                    WorkingDirectory = Path.GetDirectoryName(executable)
                }))
                {
                    if (process == null) throw new IOException("The installer process could not start.");
                    process.WaitForExit();
                    return process.ExitCode;
                }
            });
        }

        internal static bool HasWebView()
        {
            const string client = @"Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";
            foreach (var hive in new[] { RegistryHive.LocalMachine, RegistryHive.CurrentUser })
            foreach (var view in new[] { RegistryView.Registry32, RegistryView.Registry64 })
            using (var root = RegistryKey.OpenBaseKey(hive, view))
            using (var key = root.OpenSubKey(client))
            {
                Version version;
                if (key != null && Version.TryParse(key.GetValue("pv") as string, out version) && version.Major > 0) return true;
            }
            return false;
        }

        private static async Task EnsureWebView(string temporary)
        {
            if (HasWebView()) return;
            string bootstrapper = Path.Combine(temporary, "MicrosoftEdgeWebview2Setup.exe");
            ServicePointManager.SecurityProtocol = SecurityProtocolType.Tls12;
            using (var client = new WebClient())
                await client.DownloadFileTaskAsync(new Uri("https://go.microsoft.com/fwlink/p/?LinkId=2124703"), bootstrapper);
            int code = await RunProcess(bootstrapper, "/silent /install");
            if (!HasWebView()) throw new IOException("WebView2 installation failed (exit code " + code + "). Check the connection and retry.");
        }

        public static void Launch(string directory)
        {
            Launch(directory, "");
        }

        private static void Launch(string directory, string arguments)
        {
            string executable = Path.Combine(directory, Package[2] + ".exe");
            Process.Start(new ProcessStartInfo(executable, arguments) { UseShellExecute = true, WorkingDirectory = directory });
        }

        public static bool IsInstalledDirectory(string directory)
        {
            return File.Exists(Path.Combine(directory, Package[2] + ".exe"));
        }
    }
}
