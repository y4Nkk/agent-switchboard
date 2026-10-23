/** App icon contract: semantic names bound to library glyphs. Navigation uses
 * Phosphor's regular/fill pairs; Lucide chrome keeps the 1.7 line weight. Small
 * glyphs rendered below the 24-unit Lucide grid (checks, chevrons, search) scale the stroke
 * up by the same ratio so the rendered line weight is unchanged. */
import type { LucideIcon } from "lucide-react";
import {
  ArrowUp,
  ChartColumnIncreasing,
  Check,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  ChevronUp,
  ChevronsLeft,
  ChevronsRight,
  Copy,
  Download,
  Eye,
  EyeOff,
  FolderOpen,
  GripVertical,
  LoaderCircle,
  Minus,
  Network,
  Pencil,
  Pin,
  Play,
  Plus,
  RotateCw,
  ScrollText,
  Search,
  Send,
  Server,
  Sparkles,
  Square,
  Trash2,
  Upload,
  Wifi,
  X,
} from "lucide-react";

export { ArrowsLeftRightIcon as ProviderSwitchIcon } from "@phosphor-icons/react/dist/csr/ArrowsLeftRight";
export { SlidersHorizontalIcon as ClientConfigurationIcon } from "@phosphor-icons/react/dist/csr/SlidersHorizontal";
export { PuzzlePieceIcon as ExtensionsIcon } from "@phosphor-icons/react/dist/csr/PuzzlePiece";
export { ChatsIcon as SessionsIcon } from "@phosphor-icons/react/dist/csr/Chats";
export { ChartBarIcon as UsageNavigationIcon } from "@phosphor-icons/react/dist/csr/ChartBar";
export { GearSixIcon as SettingsIcon } from "@phosphor-icons/react/dist/csr/GearSix";

interface IconProps {
  size?: number;
}

function icon(Lucide: LucideIcon, size: number, strokeWidth = 1.7) {
  function Glyph({ size: override }: IconProps) {
    return <Lucide size={override ?? size} strokeWidth={strokeWidth} />;
  }
  return Glyph;
}

export const PreviewIcon = icon(Eye, 18);
export const BackToTopIcon = icon(ArrowUp, 20);
export const EyeOffIcon = icon(EyeOff, 18);
export const EditIcon = icon(Pencil, 18);
export const TrashIcon = icon(Trash2, 18);
export const PlusIcon = icon(Plus, 20);
export const MinimizeIcon = icon(Minus, 16);
export const MaximizeIcon = icon(Square, 16);
/** The two-square window-restore mark, drawn by lucide's copy glyph. */
export const RestoreIcon = icon(Copy, 16);
export const CloseIcon = icon(X, 16);
export const PinIcon = icon(Pin, 16);
export const GripIcon = icon(GripVertical, 18);
export const PlayIcon = icon(Play, 16);
export const UpdateIcon = icon(Download, 16);
export const UpdateSpinnerIcon = icon(LoaderCircle, 16);
export const ExportIcon = icon(Upload, 16);
export const UsageIcon = icon(ChartColumnIncreasing, 16);
export const ConnectivityIcon = icon(Wifi, 16);
export const RequestIcon = icon(Send, 16);
export const GatewayIcon = icon(Network, 20);
export const CollapseNavigationIcon = icon(ChevronsLeft, 20);
export const ExpandNavigationIcon = icon(ChevronsRight, 20);
export const ReloadIcon = icon(RotateCw, 20);

export const ChevronDownIcon = icon(ChevronDown, 16, 2.4);
export const ChevronLeftIcon = icon(ChevronLeft, 16, 2.4);
export const ChevronRightIcon = icon(ChevronRight, 16, 2.4);
export const ChevronUpIcon = icon(ChevronUp, 16, 2.4);
export const SearchIcon = icon(Search, 16, 2.4);
export const ScrollTextIcon = icon(ScrollText, 16, 2.4);
export const FolderOpenIcon = icon(FolderOpen, 16);
export const CheckIcon = icon(Check, 14, 3.6);
export const DashIcon = icon(Minus, 14, 3.6);

/** Extension kind glyphs, paired with the library empty state's imagery. */
export const SkillKindIcon = icon(Sparkles, 14, 2.9);
export const McpKindIcon = icon(Server, 14, 2.9);
