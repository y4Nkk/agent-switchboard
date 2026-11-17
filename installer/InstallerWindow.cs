using System;
using System.ComponentModel;
using System.Globalization;
using System.IO;
using System.Reflection;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Interop;
using System.Windows.Markup;
using System.Windows.Media;
using System.Windows.Shell;

namespace AgentSwitchboard.Installer
{
    internal enum InstallerViewState
    {
        Ready,
        Installing,
        InstallFailed,
        AwaitingLaunch,
        Launching,
        LaunchFailed,
    }

    internal sealed partial class InstallerWindow : Window
    {
        private readonly InstallerInvocation invocation;
        private readonly InstallerText copy;
        private TextBox directory;
        private TextBlock title;
        private TextBlock summary;
        private TextBlock version;
        private TextBlock diagnostic;
        private Button copyDiagnostic;
        private TextBlock existing;
        private FrameworkElement locationCard;
        private Button primary;
        private Button cancel;
        private Button close;
        private Button browse;
        private Grid progressRing;
        private System.Windows.Shapes.Path arc;
        private TextBlock percent;
        private TextBlock phase;
        private TextBlock detail;
        private readonly TextBlock[] stepTitles = new TextBlock[3];
        private readonly TextBlock[] stepStatuses = new TextBlock[3];
        private InstallationRequest installation;
        private Exception failure;
        private InstallerViewState viewState = InstallerViewState.Ready;

        internal InstallerWindow(InstallerInvocation invocation)
        {
            this.invocation = invocation;
            copy = new InstallerText();
            ExitCode = 1;
            LoadResources();
            ConfigureWindow();
            Content = BuildLayout();
            UpdateExisting();
            SourceInitialized += delegate { ApplySystemMaterial(); };
            Closing += OnClosing;
            PreviewKeyDown += OnPreviewKeyDown;
            Loaded += OnLoaded;
        }

        internal int ExitCode { get; private set; }

        private void LoadResources()
        {
            using (Stream stream = Assembly.GetExecutingAssembly().GetManifestResourceStream(
                "AgentSwitchboard.Installer.Theme.xaml"))
            {
                if (stream == null) throw new InvalidDataException("Installer theme is missing.");
                Resources = (ResourceDictionary)XamlReader.Load(stream);
            }
        }

        private void ConfigureWindow()
        {
            Title = copy.WindowTitle;
            Height = Math.Min(600, SystemParameters.WorkArea.Height - 32);
            Width = Math.Min(680, SystemParameters.WorkArea.Width - 32);
            WindowStartupLocation = WindowStartupLocation.CenterScreen;
            WindowStyle = WindowStyle.None;
            ResizeMode = ResizeMode.CanMinimize;
            Background = BackgroundBrush();
            FontFamily = Font("InterfaceFont");
            UseLayoutRounding = true;
            WindowChrome.SetWindowChrome(this, new WindowChrome
            {
                CaptionHeight = 52,
                ResizeBorderThickness = new Thickness(0),
                CornerRadius = new CornerRadius(0),
                GlassFrameThickness = new Thickness(0),
            });
        }

        private async void OnLoaded(object sender, RoutedEventArgs eventArgs)
        {
            if (viewState == InstallerViewState.Ready && invocation.IsUpdate) await InstallAsync();
            else if (viewState == InstallerViewState.Ready) primary.Focus();
        }

        private async void OnPrimaryClick(object sender, RoutedEventArgs eventArgs)
        {
            if (viewState == InstallerViewState.AwaitingLaunch || viewState == InstallerViewState.LaunchFailed)
                LaunchApplication();
            else if (viewState == InstallerViewState.Ready || viewState == InstallerViewState.InstallFailed)
                await InstallAsync();
        }

        private async System.Threading.Tasks.Task InstallAsync()
        {
            if (viewState != InstallerViewState.Ready && viewState != InstallerViewState.InstallFailed) return;

            ExitCode = 1;
            failure = null;
            try
            {
                bool update = IsUpdateTarget;
                installation = InstallationRequest.Create(directory.Text, update);
                EnterInstallingState();
                await InstallerEngine.Run(
                    installation,
                    new Progress<InstallProgress>(OnInstallProgress));
                EnterAwaitingLaunchState();
            }
            catch (Exception error)
            {
                EnterInstallFailureState(error);
            }
        }

        private void EnterInstallingState()
        {
            viewState = InstallerViewState.Installing;
            UpdateSteps();
            UpdateActions();
            directory.IsEnabled = false;
            browse.IsEnabled = false;
            locationCard.Visibility = Visibility.Collapsed;
            version.Visibility = Visibility.Collapsed;
            title.Text = copy.InstallingTitle;
            summary.Foreground = Brush("Muted");
            summary.Text = copy.InstallingStatus;
            diagnostic.Visibility = Visibility.Collapsed;
            percent.Text = "0%";
            arc.Data = ArcGeometry(0);
            phase.Text = String.Empty;
            phase.Visibility = Visibility.Collapsed;
            detail.Text = copy.ProgressFacts(copy.Version(InstallerProductMetadata.Version), null, directory.Text);
            detail.Visibility = Visibility.Visible;
            progressRing.Visibility = Visibility.Visible;
        }

        /// Progress<T> marshals every report onto the UI thread; the ring, the
        /// percentage, the phase and the data line all render from one report.
        private void OnInstallProgress(InstallProgress progress)
        {
            if (viewState != InstallerViewState.Installing) return;
            double fraction = Math.Max(0.0, Math.Min(1.0, progress.Fraction));
            percent.Text = Math.Round(fraction * 100.0).ToString(CultureInfo.CurrentCulture) + "%";
            arc.Data = ArcGeometry(fraction);
            phase.Text = copy.Phase(progress.Phase);
            phase.Visibility = Visibility.Visible;
            detail.Text = ProgressDetail(progress);
        }

        private string ProgressDetail(InstallProgress progress)
        {
            string bytes = null;
            switch (progress.Phase)
            {
                case InstallPhase.RuntimeDownload:
                    if (progress.TotalBytes.HasValue) bytes = copy.Downloaded(progress.Bytes, progress.TotalBytes.Value);
                    break;
                case InstallPhase.EngineExtract:
                    if (progress.TotalBytes.HasValue) bytes = copy.Extracted(progress.Bytes, progress.TotalBytes.Value);
                    break;
                case InstallPhase.EngineInstall:
                    bytes = copy.Written(progress.Bytes);
                    break;
            }
            return copy.ProgressFacts(
                copy.Version(InstallerProductMetadata.Version),
                bytes,
                installation.Directory);
        }

        private void EnterAwaitingLaunchState()
        {
            viewState = InstallerViewState.AwaitingLaunch;
            UpdateSteps();
            UpdateActions();
            ExitCode = 0;
            locationCard.Visibility = Visibility.Collapsed;
            version.Visibility = Visibility.Collapsed;
            progressRing.Visibility = Visibility.Collapsed;
            phase.Visibility = Visibility.Collapsed;
            detail.Visibility = Visibility.Collapsed;
            title.Text = copy.CompleteTitle(installation.Update);
            summary.Foreground = Brush("Muted");
            summary.Text = copy.CompletionPrompt(installation.Update);
            diagnostic.Visibility = Visibility.Collapsed;
            cancel.Focus();
        }

        private void EnterInstallFailureState(Exception error)
        {
            failure = error;
            viewState = InstallerViewState.InstallFailed;
            UpdateSteps();
            UpdateActions();
            System.Diagnostics.Trace.TraceWarning("Application installation: " + error);
            version.Visibility = Visibility.Collapsed;
            progressRing.Visibility = Visibility.Collapsed;
            phase.Visibility = Visibility.Collapsed;
            detail.Visibility = Visibility.Collapsed;
            locationCard.Visibility = FailureUsesLocation(error)
                ? Visibility.Visible
                : Visibility.Collapsed;
            title.Text = copy.FailureTitle(error);
            summary.Foreground = Brush("Error");
            summary.Text = copy.FailureStatus(error);
            copyDiagnostic.Content = copy.CopyDiagnostic;
            diagnostic.Text = copy.Diagnostic(error);
            diagnostic.Visibility = Visibility.Visible;
            if (FailureUsesLocation(error) && !invocation.IsUpdate && !InstallerProductMetadata.UsesMsiEngine)
            {
                directory.IsEnabled = true;
                browse.IsEnabled = true;
            }
            primary.Focus();
        }

        private static bool FailureUsesLocation(Exception error)
        {
            var installerError = error as InstallerException;
            return installerError != null
                && installerError.Kind == InstallerFailureKind.InvalidDirectory;
        }

        private bool IsUpdateTarget
        {
            get
            {
                return installation != null ? installation.Update
                    : invocation.IsUpdate || InstallerEngine.IsInstalledDirectory(directory.Text);
            }
        }

        private void UpdateActions()
        {
            bool update = IsUpdateTarget;
            bool installed = viewState == InstallerViewState.AwaitingLaunch
                || viewState == InstallerViewState.Launching || viewState == InstallerViewState.LaunchFailed;
            bool busy = viewState == InstallerViewState.Installing || viewState == InstallerViewState.Launching;
            primary.IsEnabled = !busy;
            cancel.IsEnabled = !busy;
            close.IsEnabled = !busy;
            primary.IsDefault = !installed;
            primary.Content = installed ? copy.LaunchAction(update, viewState == InstallerViewState.LaunchFailed)
                : viewState == InstallerViewState.InstallFailed ? copy.Retry : update ? copy.Update : copy.Install;
            cancel.Content = installed ? copy.Later : copy.Cancel;
            cancel.Visibility = viewState == InstallerViewState.Installing ? Visibility.Collapsed : Visibility.Visible;
        }

        private void LaunchApplication()
        {
            if (viewState != InstallerViewState.AwaitingLaunch && viewState != InstallerViewState.LaunchFailed) return;
            viewState = InstallerViewState.Launching;
            failure = null;
            UpdateSteps();
            UpdateActions();
            summary.Foreground = Brush("Muted");
            summary.Text = copy.LaunchingStatus;
            diagnostic.Visibility = Visibility.Collapsed;
            try { InstallerEngine.Launch(installation.Directory, invocation.ApplicationArguments()); }
            catch (Exception error)
            {
                failure = error;
                viewState = InstallerViewState.LaunchFailed;
                UpdateSteps();
                UpdateActions();
                System.Diagnostics.Trace.TraceWarning("Application launch: " + error);
                summary.Foreground = Brush("Error");
                summary.Text = copy.LaunchFailureStatus;
                copyDiagnostic.Content = copy.CopyDiagnostic;
                diagnostic.Text = copy.Diagnostic(error);
                diagnostic.Visibility = Visibility.Visible;
                return;
            }
            viewState = InstallerViewState.AwaitingLaunch;
            Close();
        }

        private void UpdateExisting()
        {
            bool existingInstallation = InstallerEngine.IsInstalledDirectory(directory.Text);
            existing.Text = existingInstallation
                ? copy.ExistingInstallationNotice
                : copy.NewInstallationNotice;
            if (viewState != InstallerViewState.Ready) return;
            summary.Text = copy.ReadyInstructions(IsUpdateTarget);
            UpdateSteps();
            if (primary != null) UpdateActions();
        }

        private void UpdateSteps()
        {
            bool needsAttention = viewState == InstallerViewState.InstallFailed || viewState == InstallerViewState.LaunchFailed;
            int currentStep;
            switch (viewState)
            {
                case InstallerViewState.Ready: currentStep = 1; break;
                case InstallerViewState.Installing: currentStep = 2; break;
                case InstallerViewState.InstallFailed: currentStep = FailureUsesLocation(failure) ? 1 : 2; break;
                case InstallerViewState.AwaitingLaunch:
                case InstallerViewState.Launching:
                case InstallerViewState.LaunchFailed: currentStep = 3; break;
                default: throw new InvalidOperationException("Unsupported installer view state.");
            }
            bool update = IsUpdateTarget;
            for (int index = 0; index < stepTitles.Length; index++)
            {
                bool current = index + 1 == currentStep;
                bool completed = index + 1 < currentStep;
                stepTitles[index].Text = copy.StepTitle(index + 1, update);
                stepTitles[index].FontWeight = current ? FontWeights.SemiBold : FontWeights.Normal;
                stepTitles[index].Foreground = Brush(current ? "Ink" : "Muted");
                stepStatuses[index].Text = completed ? copy.StepCompleted
                    : current ? (needsAttention ? copy.StepAttention : copy.StepCurrent) : copy.StepPending;
                stepStatuses[index].Foreground = Brush(current ? (needsAttention ? "Error" : "ActionHover") : "Muted");
            }
        }

        private void Browse(object sender, RoutedEventArgs eventArgs)
        {
            using (var dialog = new System.Windows.Forms.FolderBrowserDialog())
            {
                dialog.Description = copy.ChooseInstallLocation;
                if (System.IO.Directory.Exists(directory.Text)) dialog.SelectedPath = directory.Text;
                if (dialog.ShowDialog() == System.Windows.Forms.DialogResult.OK)
                    directory.Text = dialog.SelectedPath;
            }
        }

        private void OnPreviewKeyDown(object sender, KeyEventArgs eventArgs)
        {
            if (eventArgs.Key != Key.Escape) return;
            eventArgs.Handled = true;
            Close();
        }

        private void OnClosing(object sender, CancelEventArgs eventArgs)
        {
            if (viewState != InstallerViewState.Installing && viewState != InstallerViewState.Launching) return;
            eventArgs.Cancel = true;
            summary.Text = viewState == InstallerViewState.Installing ? copy.InstallingCannotClose : copy.LaunchingStatus;
            diagnostic.Visibility = Visibility.Collapsed;
        }

        [System.Runtime.InteropServices.DllImport("dwmapi.dll", PreserveSig = true)]
        private static extern int DwmSetWindowAttribute(
            IntPtr window,
            int attribute,
            ref int value,
            int size);

        private void ApplySystemMaterial()
        {
            var handle = new WindowInteropHelper(this).Handle;
            int round = 2;
            DwmSetWindowAttribute(handle, 33, ref round, sizeof(int));
            int noBorder = unchecked((int)0xFFFFFFFE);
            DwmSetWindowAttribute(handle, 34, ref noBorder, sizeof(int));
        }
    }
}
