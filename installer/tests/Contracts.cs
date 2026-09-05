using System;
using System.IO;
using System.Reflection;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Shell;
using AgentSwitchboard.Installer;

internal static class Contracts
{
    private static void Equal(object expected, object actual)
    {
        if (!Object.Equals(expected, actual)) throw new Exception("Expected " + expected + ", got " + actual);
    }

    [STAThread]
    public static int Main(string[] args)
    {
        if (args.Length == 2 && args[0] == "--child") return Int32.Parse(args[1]);
        string root = Path.Combine(Path.GetTempPath(), "asb-installer-test-" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(root);
        try
        {
            var options = InstallOptions.Parse("setup.exe /P /UPDATE /R /ARGS --path \"a/b c\" \"/D=app argument\"");
            Equal(true, options.Passive);
            Equal(true, options.Update);
            Equal(true, options.Restart);
            Equal(null, options.Directory);
            options.Directory = "C:\\App Folder";
            Equal("/S /UPDATE /D=C:\\App Folder", options.EngineArguments());
            Equal("\"--path\" \"a/b c\" \"/D=app argument\"", options.ApplicationArguments());
            Equal("C:\\App  Folder", InstallOptions.Parse("\"C:\\setup file.exe\" /S /D=C:\\App  Folder").Directory);
            Equal("\"trailing\\\\\"", InstallOptions.Quote("trailing\\"));
            Equal("\"a\\\"b\"", InstallOptions.Quote("a\"b"));
            var silent = InstallOptions.Parse("setup.exe /S /NS");
            silent.Directory = root;
            Equal(true, silent.Silent);
            Equal("/S /NS /D=" + root, silent.EngineArguments());
            bool invalid = false;
            try { InstallOptions.Parse("setup.exe /UNKNOWN"); } catch (ArgumentException) { invalid = true; }
            Equal(true, invalid);
            foreach (string directory in new[] { "relative", "C:relative", "\\relative", "C:\\", "C:\\bad\"path" })
            {
                invalid = false;
                try { InstallOptions.ValidateDirectory(directory); } catch (ArgumentException) { invalid = true; }
                Equal(true, invalid);
            }
            Equal(false, InstallerEngine.IsInstalledDirectory(root));
            File.WriteAllText(Path.Combine(root, "test-installer.exe"), "test");
            Equal(true, InstallerEngine.IsInstalledDirectory(root));
            Equal(23, InstallerEngine.RunProcess(Assembly.GetExecutingAssembly().Location, "--child 23").GetAwaiter().GetResult());
            var window = new InstallerWindow(new InstallOptions { Directory = root }, "0.0.0");
            Equal(WindowStyle.None, window.WindowStyle);
            Equal(ResizeMode.CanMinimize, window.ResizeMode);
            Equal(60.0, WindowChrome.GetWindowChrome(window).CaptionHeight);
            Equal(new CornerRadius(0), WindowChrome.GetWindowChrome(window).CornerRadius);
            Equal(new Thickness(0), WindowChrome.GetWindowChrome(window).ResizeBorderThickness);
            var surface = window.Content as Grid;
            Equal(true, surface != null);
            Equal(new Thickness(0), surface.Margin);
            Equal(true, surface.Background != null);
            var layout = (Grid)surface.Children[0];
            var header = (Grid)layout.Children[0];
            var minimize = (Button)header.Children[1];
            Equal("MinimizeButton", minimize.Name);
            Equal(true, WindowChrome.GetIsHitTestVisibleInChrome(minimize));
            minimize.RaiseEvent(new RoutedEventArgs(Button.ClickEvent));
            Equal(WindowState.Minimized, window.WindowState);
            window.Close();
            Console.WriteLine("Installer contracts passed: passive/silent, restart arguments, path validation, installed detection, child exit status, single edge-to-edge window surface.");
            return 0;
        }
        finally { Directory.Delete(root, true); }
    }
}
