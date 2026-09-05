using System;
using System.ComponentModel;
using System.Globalization;
using System.IO;
using System.Reflection;
using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Markup;
using System.Windows.Interop;
using System.Windows.Media;
using ShapePath = System.Windows.Shapes.Path;
using System.Windows.Shell;

namespace AgentSwitchboard.Installer
{
    internal sealed class InstallerWindow : Window
    {
        private readonly InstallOptions options;
        private readonly bool chinese = CultureInfo.CurrentUICulture.TwoLetterISOLanguageName == "zh";
        private readonly TextBox directory;
        private readonly TextBlock title;
        private readonly TextBlock status;
        private readonly TextBlock existing;
        private readonly Button primary;
        private readonly Button cancel;
        private readonly Button close;
        private readonly Button browse;
        private readonly CheckBox launch;
        private readonly ProgressBar progress;
        private bool running;
        private bool completed;
        private bool restartFailed;
        public int ExitCode { get; private set; }

        public InstallerWindow(InstallOptions options, string version)
        {
            this.options = options;
            ExitCode = 1;
            using (Stream stream = Assembly.GetExecutingAssembly().GetManifestResourceStream("AgentSwitchboard.Installer.Theme.xaml"))
                Resources = (ResourceDictionary)XamlReader.Load(stream);
            Title = "Agent Switchboard";
            Height = Math.Min(620, SystemParameters.WorkArea.Height - 24);
            Width = Math.Min(640, SystemParameters.WorkArea.Width - 24);
            WindowStartupLocation = WindowStartupLocation.CenterScreen;
            WindowStyle = WindowStyle.None;
            ResizeMode = ResizeMode.CanMinimize;
            Background = Brush("Surface");
            FontFamily = (FontFamily)Resources["InterfaceFont"];
            UseLayoutRounding = true;
            WindowChrome.SetWindowChrome(this, new WindowChrome { CaptionHeight = 60, ResizeBorderThickness = new Thickness(0), CornerRadius = new CornerRadius(0), GlassFrameThickness = new Thickness(0) });
            var frame = new Grid { Background = Brush("Surface") };
            Content = frame;
            SourceInitialized += delegate { ApplySystemMaterial(frame); };
            var layout = new Grid { Margin = new Thickness(32, 20, 32, 28) };
            frame.Children.Add(layout);
            layout.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
            layout.RowDefinitions.Add(new RowDefinition { Height = new GridLength(1, GridUnitType.Star) });
            layout.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
            var header = new Grid { Background = Brushes.Transparent };
            header.ColumnDefinitions.Add(new ColumnDefinition());
            header.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            header.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            layout.Children.Add(header);
            var brand = Text("Agent Switchboard", "BodySize");
            brand.FontWeight = FontWeights.SemiBold;
            brand.VerticalAlignment = VerticalAlignment.Center;
            header.Children.Add(brand);
            var minimize = MakeButton("−", false);
            minimize.Name = "MinimizeButton";
            minimize.MinWidth = 40;
            minimize.Padding = new Thickness(0);
            AutomationProperties.SetName(minimize, T("最小化", "Minimize"));
            WindowChrome.SetIsHitTestVisibleInChrome(minimize, true);
            minimize.Click += delegate { WindowState = WindowState.Minimized; };
            Grid.SetColumn(minimize, 1);
            header.Children.Add(minimize);
            close = MakeButton("×", false);
            close.MinWidth = 40;
            close.Padding = new Thickness(0);
            close.ToolTip = T("关闭", "Close");
            AutomationProperties.SetName(close, T("关闭安装程序", "Close installer"));
            close.Click += delegate { Close(); };
            WindowChrome.SetIsHitTestVisibleInChrome(close, true);
            Grid.SetColumn(close, 2);
            header.Children.Add(close);
            var body = new StackPanel { Margin = new Thickness(0, 24, 0, 16) };
            var scroll = new ScrollViewer { Content = body, VerticalScrollBarVisibility = ScrollBarVisibility.Hidden, HorizontalScrollBarVisibility = ScrollBarVisibility.Disabled };
            Grid.SetRow(scroll, 1);
            layout.Children.Add(scroll);
            var logo = new Grid { Width = 48, Height = 48, HorizontalAlignment = HorizontalAlignment.Left, Margin = new Thickness(0, 0, 0, 16) };
            logo.Children.Add(new ShapePath { Data = Geometry.Parse("M 23,0 C 9,4 1,14 1,24 C 1,34 9,44 23,48 Z"), Fill = Brush("Action") });
            logo.Children.Add(new ShapePath { Data = Geometry.Parse("M 26,0 C 40,4 48,14 48,24 C 48,34 40,44 26,48 Z"), Fill = Brush("Violet") });
            body.Children.Add(logo);
            title = Text(T("安装 Agent Switchboard", "Install Agent Switchboard"), "TitleSize");
            title.FontWeight = FontWeights.SemiBold;
            body.Children.Add(title);
            var versionText = Text(T("版本 ", "Version ") + version, "CaptionSize");
            versionText.Foreground = Brush("Muted");
            versionText.Margin = new Thickness(0, 6, 0, 24);
            body.Children.Add(versionText);
            body.Children.Add(Text(T("安装位置", "Install location"), "BodySize"));
            var pathRow = new Grid { Margin = new Thickness(0, 8, 0, 8) };
            pathRow.ColumnDefinitions.Add(new ColumnDefinition());
            pathRow.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            directory = new TextBox { Text = options.Directory ?? InstallerEngine.DetectDirectory() };
            AutomationProperties.SetName(directory, T("安装位置", "Install location"));
            pathRow.Children.Add(directory);
            browse = MakeButton(T("浏览…", "Browse…"), false);
            browse.Margin = new Thickness(8, 0, 0, 0);
            browse.Click += Browse;
            Grid.SetColumn(browse, 1);
            pathRow.Children.Add(browse);
            body.Children.Add(pathRow);
            existing = Text("", "CaptionSize");
            existing.Foreground = Brush("Muted");
            body.Children.Add(existing);
            directory.TextChanged += delegate { UpdateExisting(); };
            UpdateExisting();
            progress = new ProgressBar { Visibility = Visibility.Collapsed, Margin = new Thickness(0, 20, 0, 12) };
            body.Children.Add(progress);
            status = Text("", "BodySize");
            status.Margin = new Thickness(0, 12, 0, 0);
            AutomationProperties.SetLiveSetting(status, AutomationLiveSetting.Polite);
            body.Children.Add(status);
            launch = new CheckBox { Content = T("完成后启动 Agent Switchboard", "Launch Agent Switchboard when finished"), IsChecked = false, Visibility = Visibility.Collapsed, Margin = new Thickness(0, 8, 0, 0) };
            body.Children.Add(launch);
            var actions = new StackPanel { Orientation = Orientation.Horizontal, HorizontalAlignment = HorizontalAlignment.Right };
            Grid.SetRow(actions, 2);
            layout.Children.Add(actions);
            cancel = MakeButton(T("取消", "Cancel"), false);
            cancel.Click += delegate { Close(); };
            actions.Children.Add(cancel);
            primary = MakeButton(T("安装", "Install"), true);
            primary.Margin = new Thickness(12, 0, 0, 0);
            primary.IsDefault = true;
            primary.Click += async delegate { if (completed) Finish(); else await Install(); };
            actions.Children.Add(primary);
            Closing += OnClosing;
            PreviewKeyDown += delegate(object sender, KeyEventArgs e) { if (e.Key == Key.Escape) { e.Handled = true; Close(); } };
            Loaded += async delegate { if (options.Passive) await Install(); else primary.Focus(); };
        }

        private string T(string zh, string en) { return chinese ? zh : en; }
        private Brush Brush(string key) { return (Brush)Resources[key]; }
        private TextBlock Text(string value, string size) { return new TextBlock { Text = value, FontSize = (double)Resources[size] }; }
        private Button MakeButton(string label, bool main) { return new Button { Content = label, Style = (Style)Resources[main ? (object)"Primary" : typeof(Button)] }; }

        [DllImport("dwmapi.dll", PreserveSig = true)]
        private static extern int DwmSetWindowAttribute(IntPtr window, int attribute, ref int value, int size);

        private void ApplySystemMaterial(Grid frame)
        {
            var handle = new WindowInteropHelper(this).Handle;
            // DWM owns the outer contour; the WPF surface fills the client area without another frame.
            int round = 2;
            DwmSetWindowAttribute(handle, 33, ref round, sizeof(int));
            int noBorder = unchecked((int)0xFFFFFFFE);
            DwmSetWindowAttribute(handle, 34, ref noBorder, sizeof(int));
            int acrylic = 3;
            if (DwmSetWindowAttribute(handle, 38, ref acrylic, sizeof(int)) < 0) return;
            WindowChrome.GetWindowChrome(this).GlassFrameThickness = new Thickness(-1);
            HwndSource.FromHwnd(handle).CompositionTarget.BackgroundColor = Colors.Transparent;
            Background = Brushes.Transparent;
            frame.Background = Brush("GlassSurface");
        }

        private void UpdateExisting()
        {
            existing.Text = InstallerEngine.IsInstalledDirectory(directory.Text)
                ? T("此位置已安装应用，将更新程序并保留应用数据。", "The application is installed here. Setup will update it and keep application data.")
                : T("仅为当前 Windows 用户安装。", "Install for the current Windows user only.");
        }

        private void Browse(object sender, RoutedEventArgs e)
        {
            using (var dialog = new System.Windows.Forms.FolderBrowserDialog())
            {
                dialog.Description = T("选择安装位置", "Choose install location");
                dialog.SelectedPath = directory.Text;
                if (dialog.ShowDialog() == System.Windows.Forms.DialogResult.OK) directory.Text = dialog.SelectedPath;
            }
        }

        private async System.Threading.Tasks.Task Install()
        {
            if (running) return;
            running = true;
            ExitCode = 1;
            directory.IsEnabled = browse.IsEnabled = primary.IsEnabled = cancel.IsEnabled = close.IsEnabled = false;
            progress.Visibility = Visibility.Visible;
            progress.IsIndeterminate = SystemParameters.ClientAreaAnimation;
            status.Foreground = Brush("Muted");
            status.Text = T("正在安装，请保持此窗口打开。", "Installing. Keep this window open until setup finishes.");
            try
            {
                options.Directory = directory.Text;
                var result = await InstallerEngine.Run(options);
                ExitCode = result.ExitCode;
                if (ExitCode != 0) throw new InvalidOperationException(T("安装程序退出代码：", "Installer exit code: ") + ExitCode);
                completed = true;
                title.Text = T("安装完成", "Installation complete");
                status.Text = T("Agent Switchboard 已准备就绪。", "Agent Switchboard is ready.");
                restartFailed = result.LaunchError != null;
                if (restartFailed) status.Text = T("已安装，但自动启动失败：", "Installed, but automatic launch failed: ") + result.LaunchError;
                primary.Content = T("完成", "Finish");
                cancel.Visibility = Visibility.Collapsed;
                launch.Visibility = options.Restart && !restartFailed ? Visibility.Collapsed : Visibility.Visible;
            }
            catch (Exception error)
            {
                if (ExitCode == 0) ExitCode = 1;
                status.Foreground = Brush("Error");
                status.Text = T("安装未完成。", "Installation did not complete. ") + error.Message;
                primary.Content = T("重试", "Retry");
                directory.IsEnabled = browse.IsEnabled = true;
            }
            finally
            {
                running = false;
                progress.IsIndeterminate = false;
                progress.Visibility = Visibility.Collapsed;
                primary.IsEnabled = cancel.IsEnabled = close.IsEnabled = true;
                primary.Focus();
            }
            if (options.Passive && completed && !restartFailed) Close();
        }

        private void Finish()
        {
            try { if (launch.IsChecked == true && (!options.Restart || restartFailed)) InstallerEngine.Launch(options.Directory); }
            catch (Exception error) { status.Foreground = Brush("Error"); status.Text = T("已安装，但启动失败：", "Installed, but could not launch: ") + error.Message; launch.IsChecked = false; return; }
            Close();
        }

        private void OnClosing(object sender, CancelEventArgs e)
        {
            if (!running) return;
            e.Cancel = true;
            status.Text = T("正在安装，完成后即可关闭。", "Installation is running. You can close this window when it finishes.");
        }
    }
}
