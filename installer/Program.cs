using System;
using System.Windows;

namespace AgentSwitchboard.Installer
{
    internal static class Program
    {
        [STAThread]
        private static int Main(string[] args)
        {
            bool silent = Array.IndexOf(args, "/S") >= 0;
            try
            {
                var options = InstallOptions.Parse(Environment.CommandLine);
                if (options.Directory == null) options.Directory = InstallerEngine.DetectDirectory();
                if (options.Silent) return InstallerEngine.Run(options).GetAwaiter().GetResult().ExitCode;
                var application = new Application();
                var window = new InstallerWindow(options, InstallerEngine.Package[1]);
                application.Run(window);
                return window.ExitCode;
            }
            catch (Exception error)
            {
                if (!silent) MessageBox.Show(error.Message, "Agent Switchboard", MessageBoxButton.OK, MessageBoxImage.Error);
                return 2;
            }
        }
    }
}
