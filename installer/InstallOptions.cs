using System;
using System.Collections.Generic;
using System.IO;
using System.Text;
using System.Runtime.InteropServices;

namespace AgentSwitchboard.Installer
{
    public sealed class InstallOptions
    {
        public string Directory { get; set; }
        public bool Silent { get; set; }
        public bool Passive { get; set; }
        public bool Restart { get; set; }
        public bool Update { get; set; }
        public bool NoShortcuts { get; set; }
        public readonly List<string> LaunchArguments = new List<string>();

        [DllImport("shell32.dll", SetLastError = true)]
        private static extern IntPtr CommandLineToArgvW([MarshalAs(UnmanagedType.LPWStr)] string commandLine, out int count);
        [DllImport("kernel32.dll")]
        private static extern IntPtr LocalFree(IntPtr memory);

        public static InstallOptions Parse(string commandLine)
        {
            var result = new InstallOptions();
            int directory = DirectoryOffset(commandLine);
            if (directory >= 0)
            {
                result.Directory = commandLine.Substring(directory);
                commandLine = commandLine.Substring(0, directory - 3);
            }
            int count;
            IntPtr memory = CommandLineToArgvW(commandLine, out count);
            if (memory == IntPtr.Zero) throw new ArgumentException("Invalid installer command line.");
            var args = new string[Math.Max(0, count - 1)];
            try { for (int i = 1; i < count; i++) args[i - 1] = Marshal.PtrToStringUni(Marshal.ReadIntPtr(memory, i * IntPtr.Size)); }
            finally { LocalFree(memory); }
            bool launch = false;
            for (int i = 0; i < args.Length; i++)
            {
                string arg = args[i];
                if (launch) { result.LaunchArguments.Add(arg); continue; }
                switch (arg)
                {
                    case "/S": result.Silent = true; break;
                    case "/P": result.Passive = true; break;
                    case "/R": result.Restart = true; break;
                    case "/UPDATE": result.Update = true; break;
                    case "/NS": result.NoShortcuts = true; break;
                    case "/ARGS": launch = true; break;
                    default: throw new ArgumentException("Unknown installer option: " + arg);
                }
            }
            return result;
        }

        private static int DirectoryOffset(string raw)
        {
            bool quoted = false;
            int slashes = 0;
            for (int i = 0; i < raw.Length; i++)
            {
                if (raw[i] == '"' && slashes % 2 == 0) quoted = !quoted;
                if (!quoted && raw[i] == '/' && i > 0 && Char.IsWhiteSpace(raw[i - 1]))
                {
                    if (raw.Substring(i).StartsWith("/ARGS", StringComparison.Ordinal)
                        && (i + 5 == raw.Length || Char.IsWhiteSpace(raw[i + 5]))) return -1;
                    if (raw.Substring(i).StartsWith("/D=", StringComparison.Ordinal)) return i + 3;
                }
                slashes = raw[i] == '\\' ? slashes + 1 : 0;
            }
            return -1;
        }

        public string EngineArguments()
        {
            ValidateDirectory(Directory);
            var args = new StringBuilder("/S");
            if (Update) args.Append(" /UPDATE");
            if (NoShortcuts) args.Append(" /NS");
            args.Append(" /D=").Append(Directory);
            return args.ToString();
        }

        public string ApplicationArguments()
        {
            return String.Join(" ", LaunchArguments.ConvertAll(Quote));
        }

        internal static string Quote(string value)
        {
            var result = new StringBuilder("\"");
            int slashes = 0;
            foreach (char c in value)
            {
                if (c == '\\') { slashes++; continue; }
                if (c == '"') result.Append('\\', slashes * 2 + 1).Append(c);
                else result.Append('\\', slashes).Append(c);
                slashes = 0;
            }
            return result.Append('\\', slashes * 2).Append('"').ToString();
        }

        public static void ValidateDirectory(string directory)
        {
            if (String.IsNullOrWhiteSpace(directory) || !Path.IsPathRooted(directory)
                || directory.IndexOfAny(new[] { '"', '\r', '\n' }) >= 0)
                throw new ArgumentException("Select an absolute installation directory.");
            string full = Path.GetFullPath(directory).TrimEnd(Path.DirectorySeparatorChar);
            string root = Path.GetPathRoot(directory);
            if (root == "\\" || root.EndsWith(":"))
                throw new ArgumentException("Select an absolute installation directory.");
            if (full == Path.GetPathRoot(directory).TrimEnd(Path.DirectorySeparatorChar))
                throw new ArgumentException("Select an application folder, not a drive root.");
        }
    }
}
