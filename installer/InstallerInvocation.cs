using System;
using System.Text;

namespace AgentSwitchboard.Installer
{
    internal enum InstallerInvocationKind
    {
        Interactive,
        Updater,
    }

    internal sealed class InstallerInvocation
    {
        private readonly string[] launchArguments;

        private InstallerInvocation(InstallerInvocationKind kind, string[] launchArguments)
        {
            Kind = kind;
            this.launchArguments = launchArguments;
        }

        internal InstallerInvocationKind Kind { get; private set; }

        internal bool IsUpdate
        {
            get { return Kind == InstallerInvocationKind.Updater; }
        }

        internal static InstallerInvocation Parse(string[] arguments)
        {
            if (arguments == null || arguments.Length == 0)
                return new InstallerInvocation(InstallerInvocationKind.Interactive, new string[0]);

            if (!IsCurrentUpdaterInvocation(arguments))
                throw new ArgumentException("Unsupported installer invocation.");

            var launchArguments = new string[arguments.Length - 4];
            Array.Copy(arguments, 4, launchArguments, 0, launchArguments.Length);
            return new InstallerInvocation(InstallerInvocationKind.Updater, launchArguments);
        }

        internal string ApplicationArguments()
        {
            if (launchArguments.Length == 0) return String.Empty;

            var result = new StringBuilder();
            for (int index = 0; index < launchArguments.Length; index++)
            {
                if (index > 0) result.Append(' ');
                result.Append(Quote(launchArguments[index]));
            }
            return result.ToString();
        }

        private static bool IsCurrentUpdaterInvocation(string[] arguments)
        {
            // tauri-plugin-updater 2.11.0 emits this exact Windows NSIS protocol for
            // the product's configured passive update and restart request. The
            // installer confirms that request after installation. This is a
            // current integration contract, not a general NSIS command-line parser.
            return arguments.Length >= 4
                && String.Equals(arguments[0], "/P", StringComparison.Ordinal)
                && String.Equals(arguments[1], "/UPDATE", StringComparison.Ordinal)
                && String.Equals(arguments[2], "/R", StringComparison.Ordinal)
                && String.Equals(arguments[3], "/ARGS", StringComparison.Ordinal);
        }

        private static string Quote(string value)
        {
            var result = new StringBuilder("\"");
            int slashes = 0;
            foreach (char character in value)
            {
                if (character == '\\')
                {
                    slashes++;
                    continue;
                }
                if (character == '"') result.Append('\\', slashes * 2 + 1).Append(character);
                else result.Append('\\', slashes).Append(character);
                slashes = 0;
            }
            return result.Append('\\', slashes * 2).Append('"').ToString();
        }
    }
}
