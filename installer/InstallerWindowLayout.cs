using System;
using System.Diagnostics;
using System.IO;
using System.Reflection;
using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Data;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Shell;

namespace AgentSwitchboard.Installer
{
    internal sealed partial class InstallerWindow
    {
        private const double RingSize = 116;
        private const double RingStroke = 8;

        private FrameworkElement BuildLayout()
        {
            var layout = new Grid { Margin = new Thickness(32, 8, 32, 28) };
            layout.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
            layout.RowDefinitions.Add(new RowDefinition { Height = new GridLength(1, GridUnitType.Star) });
            layout.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });

            layout.Children.Add(BuildHeader());
            var body = new ScrollViewer {
                Content = BuildBody(), VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
                HorizontalScrollBarVisibility = ScrollBarVisibility.Disabled,
            };
            Grid.SetRow(body, 1);
            layout.Children.Add(body);

            var footer = BuildFooter();
            Grid.SetRow(footer, 2);
            layout.Children.Add(footer);
            return layout;
        }

        private Grid BuildHeader()
        {
            var header = new Grid { Height = 44, Background = Brushes.Transparent };
            header.ColumnDefinitions.Add(new ColumnDefinition());
            header.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            header.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });

            var brand = new StackPanel
            {
                Orientation = Orientation.Horizontal,
                VerticalAlignment = VerticalAlignment.Center,
            };
            AutomationProperties.SetName(brand, copy.BrandName + " " + copy.BrandContext);
            var brandMark = CreateBrandMark(20);
            brandMark.Margin = new Thickness(0, 0, 9, 0);
            brand.Children.Add(brandMark);

            var brandName = Text(copy.BrandName, "LabelSize");
            brandName.FontWeight = FontWeights.SemiBold;
            brand.Children.Add(brandName);

            var brandContext = Text(copy.BrandContext, "CaptionSize");
            brandContext.Foreground = Brush("Muted");
            brandContext.Margin = new Thickness(8, 1, 0, 0);
            brand.Children.Add(brandContext);
            header.Children.Add(brand);

            var minimize = MakeTitlebarButton("−", false);
            minimize.Name = "MinimizeButton";
            minimize.ToolTip = copy.Minimize;
            AutomationProperties.SetName(minimize, copy.Minimize);
            WindowChrome.SetIsHitTestVisibleInChrome(minimize, true);
            minimize.Click += delegate { WindowState = WindowState.Minimized; };
            Grid.SetColumn(minimize, 1);
            header.Children.Add(minimize);

            close = MakeTitlebarButton("×", true);
            close.ToolTip = copy.Close;
            AutomationProperties.SetName(close, copy.Close);
            WindowChrome.SetIsHitTestVisibleInChrome(close, true);
            close.Click += delegate { Close(); };
            Grid.SetColumn(close, 2);
            header.Children.Add(close);
            return header;
        }

        private FrameworkElement BuildBody()
        {
            var body = new StackPanel
            {
                HorizontalAlignment = HorizontalAlignment.Stretch,
                Margin = new Thickness(0, 22, 0, 20),
            };
            body.Children.Add(BuildHero());
            body.Children.Add(BuildSteps());
            locationCard = BuildLocationCard();
            body.Children.Add(locationCard);
            body.Children.Add(BuildProgressArea());
            return body;
        }

        private FrameworkElement BuildHero()
        {
            var hero = new StackPanel
            {
                HorizontalAlignment = HorizontalAlignment.Center,
                Margin = new Thickness(0, 0, 0, 24),
            };
            title = Text(copy.ReadyTitle, "TitleSize");
            title.FontFamily = Font("DisplayFont");
            title.FontWeight = FontWeights.SemiBold;
            title.TextAlignment = TextAlignment.Center;
            title.HorizontalAlignment = HorizontalAlignment.Center;
            hero.Children.Add(title);

            summary = Text(String.Empty, "LeadSize");
            summary.Foreground = Brush("Muted");
            summary.LineHeight = 23;
            summary.TextAlignment = TextAlignment.Center;
            summary.MaxWidth = 440;
            summary.Margin = new Thickness(0, 10, 0, 0);
            hero.Children.Add(summary);

            AutomationProperties.SetLiveSetting(summary, AutomationLiveSetting.Polite);

            version = Text(copy.Version(InstallerProductMetadata.Version), "CaptionSize");
            version.Foreground = Brush("Muted");
            version.TextAlignment = TextAlignment.Center;
            version.Margin = new Thickness(0, 10, 0, 0);
            hero.Children.Add(version);
            return hero;
        }

        private FrameworkElement BuildSteps()
        {
            var steps = new Grid { Margin = new Thickness(0, 0, 0, 24) };
            AutomationProperties.SetName(steps, copy.StepsHeading);
            for (int index = 0; index < stepTitles.Length; index++)
            {
                steps.ColumnDefinitions.Add(new ColumnDefinition());
                var item = new StackPanel { Margin = new Thickness(6, 0, 6, 0) };
                var label = Text(String.Empty, "LabelSize");
                label.TextAlignment = TextAlignment.Center;
                stepTitles[index] = label;
                item.Children.Add(label);
                var status = Text(String.Empty, "CaptionSize");
                status.TextAlignment = TextAlignment.Center;
                status.Foreground = Brush("Muted");
                status.Margin = new Thickness(0, 5, 0, 0);
                stepStatuses[index] = status;
                item.Children.Add(status);
                Grid.SetColumn(item, index);
                steps.Children.Add(item);
            }
            return steps;
        }

        private FrameworkElement BuildLocationCard()
        {
            var card = new Border { Style = (Style)Resources["SectionCard"] };
            var content = new StackPanel();
            card.Child = content;

            var label = Text(copy.InstallLocation, "BodySize");
            label.FontWeight = FontWeights.SemiBold;
            content.Children.Add(label);

            var pathRow = new Grid { Margin = new Thickness(0, 12, 0, 0) };
            pathRow.ColumnDefinitions.Add(new ColumnDefinition());
            pathRow.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            directory = new TextBox { Text = InstallerEngine.DetectDirectory() };
            AutomationProperties.SetName(directory, copy.InstallLocation);
            pathRow.Children.Add(directory);

            browse = MakeButton(copy.ChooseLocation, false);
            browse.Margin = new Thickness(8, 0, 0, 0);
            browse.Click += Browse;
            Grid.SetColumn(browse, 1);
            pathRow.Children.Add(browse);
            content.Children.Add(pathRow);

            existing = Text(String.Empty, "CaptionSize");
            existing.Foreground = Brush("Muted");
            existing.Margin = new Thickness(0, 12, 0, 0);
            content.Children.Add(existing);
            directory.TextChanged += delegate { UpdateExisting(); };
            UpdateExisting();

            // Two read-only flows share this card: the updater invocation
            // (it reinstalls over the detected location) and the MSI engine
            // (WiX owns the per-machine directory).
            if (invocation.Kind == InstallerInvocationKind.Updater
                || InstallerProductMetadata.UsesMsiEngine)
            {
                directory.IsEnabled = false;
                browse.Visibility = Visibility.Collapsed;
            }
            return card;
        }

        private FrameworkElement BuildProgressArea()
        {
            var content = new StackPanel { Margin = new Thickness(0, 18, 0, 0) };
            content.Children.Add(BuildProgressRing());

            phase = Text(String.Empty, "LabelSize");
            phase.Foreground = Brush("Muted");
            phase.HorizontalAlignment = HorizontalAlignment.Center;
            phase.Margin = new Thickness(0, 14, 0, 0);
            phase.Visibility = Visibility.Collapsed;
            content.Children.Add(phase);

            detail = Text(String.Empty, "CaptionSize");
            detail.Foreground = Brush("Muted");
            detail.HorizontalAlignment = HorizontalAlignment.Center;
            detail.Margin = new Thickness(0, 4, 0, 0);
            detail.MaxWidth = 540;
            detail.TextTrimming = TextTrimming.CharacterEllipsis;
            detail.TextWrapping = TextWrapping.NoWrap;
            detail.Visibility = Visibility.Collapsed;
            content.Children.Add(detail);

            diagnostic = Text(String.Empty, "CaptionSize");
            diagnostic.Foreground = Brush("Muted");
            diagnostic.Visibility = Visibility.Collapsed;
            diagnostic.Margin = new Thickness(0, 6, 0, 0);
            content.Children.Add(diagnostic);
            copyDiagnostic = MakeButton(copy.CopyDiagnostic, false);
            copyDiagnostic.Margin = new Thickness(0, 8, 0, 0);
            copyDiagnostic.HorizontalAlignment = HorizontalAlignment.Left;
            copyDiagnostic.SetBinding(UIElement.VisibilityProperty, new Binding("Visibility") { Source = diagnostic });
            copyDiagnostic.Click += delegate {
                try { Clipboard.SetText(diagnostic.Text); copyDiagnostic.Content = copy.CopiedDiagnostic; }
                catch (Exception error) { Trace.TraceWarning("Copy diagnostic: " + error.Message); copyDiagnostic.Content = copy.CopyDiagnosticFailed; }
            };
            content.Children.Add(copyDiagnostic);

            return content;
        }

        private FrameworkElement BuildFooter()
        {
            var footer = new Grid { Margin = new Thickness(0, 16, 0, 0) };
            footer.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
            footer.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });

            var actions = new StackPanel
            {
                Orientation = Orientation.Horizontal,
                HorizontalAlignment = HorizontalAlignment.Right,
                Margin = new Thickness(0, 16, 0, 0),
            };
            Grid.SetRow(actions, 1);
            footer.Children.Add(actions);

            cancel = MakeButton(copy.Cancel, false);
            cancel.Click += delegate { Close(); };
            actions.Children.Add(cancel);

            primary = MakeButton(copy.Install, true);
            primary.Margin = new Thickness(10, 0, 0, 0);
            primary.MinWidth = 104;
            primary.IsDefault = true;
            primary.Click += OnPrimaryClick;
            actions.Children.Add(primary);
            return footer;
        }

        /// The install progress ring: a hairline track, the accent arc that
        /// opens clockwise from 12 o'clock, and the live percentage in the
        /// center. Deterministic — every state change comes from a progress
        /// report, so it renders identically with system animations off.
        private FrameworkElement BuildProgressRing()
        {
            var ring = new Grid
            {
                Width = RingSize,
                Height = RingSize,
                HorizontalAlignment = HorizontalAlignment.Center,
                Visibility = Visibility.Collapsed,
            };
            double radius = (RingSize - RingStroke) / 2.0;
            ring.Children.Add(new System.Windows.Shapes.Path
            {
                Data = CircleGeometry(radius),
                Stroke = Brush("Line"),
                StrokeThickness = RingStroke,
            });
            arc = new System.Windows.Shapes.Path
            {
                Data = new StreamGeometry(),
                Stroke = Brush("Action"),
                StrokeThickness = RingStroke,
                StrokeStartLineCap = PenLineCap.Round,
                StrokeEndLineCap = PenLineCap.Round,
            };
            ring.Children.Add(arc);
            percent = new TextBlock
            {
                Text = "0%",
                FontSize = 26,
                FontFamily = Font("DisplayFont"),
                FontWeight = FontWeights.SemiBold,
                HorizontalAlignment = HorizontalAlignment.Center,
                VerticalAlignment = VerticalAlignment.Center,
            };
            ring.Children.Add(percent);
            progressRing = ring;
            return ring;
        }

        private static Geometry CircleGeometry(double radius)
        {
            double center = radius + RingStroke / 2.0;
            var geometry = new EllipseGeometry(new Point(center, center), radius, radius);
            geometry.Freeze();
            return geometry;
        }

        /// The arc for `fraction` (0..1): a clockwise sweep starting at the
        /// top; near-zero collapses to an empty geometry, near-one to the full
        /// circle, avoiding degenerate single-point arcs at both ends.
        internal static Geometry ArcGeometry(double fraction)
        {
            double radius = (RingSize - RingStroke) / 2.0;
            if (fraction <= 0.002) return new StreamGeometry();
            if (fraction >= 0.998) return CircleGeometry(radius);
            double angle = 2.0 * Math.PI * fraction;
            double center = radius + RingStroke / 2.0;
            var figure = new PathFigure
            {
                StartPoint = new Point(center, center - radius),
                IsClosed = false,
            };
            figure.Segments.Add(new ArcSegment
            {
                Point = new Point(
                    center + radius * Math.Sin(angle),
                    center - radius * Math.Cos(angle)),
                Size = new Size(radius, radius),
                IsLargeArc = angle > Math.PI,
                SweepDirection = SweepDirection.Clockwise,
            });
            var geometry = new PathGeometry(new[] { figure });
            geometry.Freeze();
            return geometry;
        }

        private Brush BackgroundBrush()
        {
            try
            {
                using (Stream stream = Assembly.GetExecutingAssembly().GetManifestResourceStream(
                    "AgentSwitchboard.Installer.Assets.InstallerBackground.wdp"))
                {
                    if (stream == null) return Brush("Surface");
                    var decoder = new WmpBitmapDecoder(
                        stream,
                        BitmapCreateOptions.None,
                        BitmapCacheOption.OnLoad);
                    if (decoder.Frames.Count == 0) return Brush("Surface");
                    var image = new ImageBrush(decoder.Frames[0])
                    {
                        Stretch = Stretch.UniformToFill,
                        AlignmentX = AlignmentX.Center,
                        AlignmentY = AlignmentY.Center,
                    };
                    image.Freeze();
                    return image;
                }
            }
            catch (Exception error)
            {
                Trace.TraceWarning("Installer background: " + error.Message);
                return Brush("Surface");
            }
        }

        private static FrameworkElement CreateBrandMark(double size)
        {
            using (Stream stream = Assembly.GetExecutingAssembly().GetManifestResourceStream(
                "AgentSwitchboard.Installer.Assets.BrandIcon.png"))
            {
                if (stream == null) throw new InvalidOperationException("Installer brand icon is missing.");
                var decoder = new PngBitmapDecoder(
                    stream, BitmapCreateOptions.PreservePixelFormat, BitmapCacheOption.OnLoad);
                var source = decoder.Frames[0];
                source.Freeze();
                var mark = new Image
                {
                    Width = size, Height = size, Source = source,
                    Stretch = Stretch.Uniform, SnapsToDevicePixels = true,
                };
                RenderOptions.SetBitmapScalingMode(mark, BitmapScalingMode.HighQuality);
                return mark;
            }
        }

        private Button MakeButton(string label, bool primaryButton)
        {
            return new Button
            {
                Content = label,
                Style = (Style)Resources[primaryButton ? (object)"Primary" : typeof(Button)],
            };
        }

        private Button MakeTitlebarButton(string glyph, bool closeButton)
        {
            var icon = Text(glyph, "BodySize");
            icon.FontFamily = new FontFamily("Segoe UI Symbol");
            icon.FontSize = 16;
            icon.FontWeight = FontWeights.SemiBold;
            icon.HorizontalAlignment = HorizontalAlignment.Center;
            icon.VerticalAlignment = VerticalAlignment.Center;
            icon.TextAlignment = TextAlignment.Center;
            icon.SetBinding(TextBlock.ForegroundProperty, new Binding("Foreground")
            {
                RelativeSource = new RelativeSource(RelativeSourceMode.FindAncestor, typeof(Button), 1),
            });
            return new Button
            {
                Content = icon,
                Style = (Style)Resources[closeButton ? "TitlebarCloseButton" : "TitlebarButton"],
            };
        }

        private TextBlock Text(string value, string size)
        {
            return new TextBlock { Text = value, FontSize = (double)Resources[size] };
        }

        private Brush Brush(string key)
        {
            return (Brush)Resources[key];
        }

        private FontFamily Font(string key)
        {
            return (FontFamily)Resources[key];
        }
    }
}
