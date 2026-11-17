using System;
using System.Diagnostics;
using System.IO;
using Microsoft.Win32;

namespace AgentSwitchboard.Installer
{
    internal static partial class InstallerEngine
    {
        private enum InstalledDirectoryState
        {
            Present,
            Missing,
            Unknown,
        }

        internal static string DetectDirectory()
        {
            if (InstallerProductMetadata.UsesMsiEngine)
            {
                // The WiX engine installs per-machine under Program Files; the
                // Windows Installer uninstall entry is the only live source.
                string located = TryReadMsiInstallLocation();
                if (!String.IsNullOrWhiteSpace(located)) return located;
                return Path.Combine(
                    Environment.GetFolderPath(Environment.SpecialFolder.ProgramFiles),
                    InstallerProductMetadata.ProductName);
            }
            string installed = TryReadInstalledDirectory();
            if (!String.IsNullOrWhiteSpace(installed))
            {
                switch (GetInstalledDirectoryState(installed))
                {
                    case InstalledDirectoryState.Present:
                        return installed;
                    case InstalledDirectoryState.Unknown:
                        return installed;
                    default:
                        RemoveStaleInstalledDirectory();
                        break;
                }
            }
            return Path.Combine(
                Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
                InstallerProductMetadata.ProductName);
        }

        internal static bool IsInstalledDirectory(string directory)
        {
            return GetInstalledDirectoryState(directory) == InstalledDirectoryState.Present;
        }

        private static InstalledDirectoryState GetInstalledDirectoryState(string directory)
        {
            if (String.IsNullOrWhiteSpace(directory)) return InstalledDirectoryState.Missing;
            try
            {
                string application = Path.Combine(
                    directory,
                    InstallerProductMetadata.ApplicationFileStem + ".exe");
                FileAttributes attributes = File.GetAttributes(application);
                return (attributes & FileAttributes.Directory) == 0
                    ? InstalledDirectoryState.Present
                    : InstalledDirectoryState.Missing;
            }
            catch (FileNotFoundException)
            {
                return InstalledDirectoryState.Missing;
            }
            catch (DirectoryNotFoundException)
            {
                return InstalledDirectoryState.Missing;
            }
            catch (ArgumentException)
            {
                return InstalledDirectoryState.Missing;
            }
            catch (NotSupportedException)
            {
                return InstalledDirectoryState.Missing;
            }
            catch (PathTooLongException)
            {
                return InstalledDirectoryState.Missing;
            }
            catch (IOException error)
            {
                Trace.TraceWarning("Installer directory lookup: " + error.Message);
                return InstalledDirectoryState.Unknown;
            }
            catch (UnauthorizedAccessException error)
            {
                Trace.TraceWarning("Installer directory lookup: " + error.Message);
                return InstalledDirectoryState.Unknown;
            }
            catch (System.Security.SecurityException error)
            {
                Trace.TraceWarning("Installer directory lookup: " + error.Message);
                return InstalledDirectoryState.Unknown;
            }
        }

        private static string RegistryPath
        {
            get
            {
                return "Software\\" + InstallerProductMetadata.ApplicationFileStem
                    + "\\" + InstallerProductMetadata.ProductName;
            }
        }

        private static string TryReadInstalledDirectory()
        {
            try
            {
                using (var hive = RegistryKey.OpenBaseKey(RegistryHive.CurrentUser, RegistryView.Registry64))
                using (var key = hive.OpenSubKey(RegistryPath))
                    return key == null ? null : key.GetValue(null) as string;
            }
            catch (IOException error)
            {
                Trace.TraceWarning("Installer directory lookup: " + error.Message);
                return null;
            }
            catch (UnauthorizedAccessException error)
            {
                Trace.TraceWarning("Installer directory lookup: " + error.Message);
                return null;
            }
            catch (System.Security.SecurityException error)
            {
                Trace.TraceWarning("Installer directory lookup: " + error.Message);
                return null;
            }
        }

        /// Windows Installer owns the uninstall entry of the per-machine MSI;
        /// the display name is the stable join key the WiX package carries.
        private static string TryReadMsiInstallLocation()
        {
            try
            {
                const string uninstall = @"Software\Microsoft\Windows\CurrentVersion\Uninstall";
                using (var hive = RegistryKey.OpenBaseKey(RegistryHive.LocalMachine, RegistryView.Registry64))
                using (var root = hive.OpenSubKey(uninstall))
                {
                    if (root == null) return null;
                    foreach (var subKeyName in root.GetSubKeyNames())
                    {
                        using (var key = root.OpenSubKey(subKeyName))
                        {
                            if (key == null) continue;
                            if (!String.Equals(
                                    key.GetValue("DisplayName") as string,
                                    InstallerProductMetadata.ProductName,
                                    StringComparison.OrdinalIgnoreCase)) continue;
                            string location = key.GetValue("InstallLocation") as string;
                            if (!String.IsNullOrWhiteSpace(location)) return location;
                        }
                    }
                }
                return null;
            }
            catch (IOException error)
            {
                Trace.TraceWarning("Installer directory lookup: " + error.Message);
                return null;
            }
            catch (UnauthorizedAccessException error)
            {
                Trace.TraceWarning("Installer directory lookup: " + error.Message);
                return null;
            }
            catch (System.Security.SecurityException error)
            {
                Trace.TraceWarning("Installer directory lookup: " + error.Message);
                return null;
            }
        }

        private static void RemoveStaleInstalledDirectory()
        {
            try
            {
                using (var hive = RegistryKey.OpenBaseKey(RegistryHive.CurrentUser, RegistryView.Registry64))
                    hive.DeleteSubKeyTree(RegistryPath, false);
            }
            catch (IOException error)
            {
                Trace.TraceWarning("Installer stale-directory cleanup: " + error.Message);
            }
            catch (UnauthorizedAccessException error)
            {
                Trace.TraceWarning("Installer stale-directory cleanup: " + error.Message);
            }
            catch (System.Security.SecurityException error)
            {
                Trace.TraceWarning("Installer stale-directory cleanup: " + error.Message);
            }
        }

    }
}
