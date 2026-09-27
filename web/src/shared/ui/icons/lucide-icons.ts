// 图标来自 lucide-react，保持原始几何；尺寸阶梯与光学描边由 withIconDefaults 统一。
import {
  Activity as LucideActivity,
  AlertCircle as LucideAlertCircle,
  AlignCenter as LucideAlignCenter,
  AlignLeft as LucideAlignLeft,
  AlignRight as LucideAlignRight,
  Archive as LucideArchive,
  ArchiveRestore as LucideArchiveRestore,
  ArrowDown as LucideArrowDown,
  ArrowDownToLine as LucideArrowDownToLine,
  ArrowLeft as LucideArrowLeft,
  ArrowLeftRight as LucideArrowLeftRight,
  ArrowLeftToLine as LucideArrowLeftToLine,
  ArrowRight as LucideArrowRight,
  ArrowRightToLine as LucideArrowRightToLine,
  ArrowUp as LucideArrowUp,
  ArrowUpFromLine as LucideArrowUpFromLine,
  ArrowUpToLine as LucideArrowUpToLine,
  Ban as LucideBan,
  BarChart3 as LucideBarChart3,
  Blocks as LucideBlocks,
  Bold as LucideBold,
  BookMarked as LucideBookMarked,
  BookOpen as LucideBookOpen,
  Bot as LucideBot,
  Box as LucideBox,
  Braces as LucideBraces,
  Brain as LucideBrain,
  BrainCircuit as LucideBrainCircuit,
  Cable as LucideCable,
  Calculator as LucideCalculator,
  CalendarClock as LucideCalendarClock,
  CalendarPlus as LucideCalendarPlus,
  Check as LucideCheck,
  CheckCheck as LucideCheckCheck,
  CheckCircle2 as LucideCheckCircle2,
  CheckSquare as LucideCheckSquare,
  CheckSquare2 as LucideCheckSquare2,
  Cherry as LucideCherry,
  ChevronDown as LucideChevronDown,
  ChevronLeft as LucideChevronLeft,
  ChevronRight as LucideChevronRight,
  ChevronsDownUp as LucideChevronsDownUp,
  ChevronsLeft as LucideChevronsLeft,
  ChevronsLeftRight as LucideChevronsLeftRight,
  ChevronsRight as LucideChevronsRight,
  ChevronsUpDown as LucideChevronsUpDown,
  ChevronUp as LucideChevronUp,
  Circle as LucideCircle,
  CircleAlert as LucideCircleAlert,
  CircleCheck as LucideCircleCheck,
  CircleDashed as LucideCircleDashed,
  CircleDot as LucideCircleDot,
  CircleGauge as LucideCircleGauge,
  CircleStop as LucideCircleStop,
  CircleX as LucideCircleX,
  Clipboard as LucideClipboard,
  ClipboardCopy as LucideClipboardCopy,
  ClipboardList as LucideClipboardList,
  ClipboardPaste as LucideClipboardPaste,
  Clock as LucideClock,
  Clock3 as LucideClock3,
  CloudDownload as LucideCloudDownload,
  CloudUpload as LucideCloudUpload,
  Code as LucideCode,
  Code2 as LucideCode2,
  Columns2 as LucideColumns2,
  Columns3 as LucideColumns3,
  Compass as LucideCompass,
  Copy as LucideCopy,
  Cpu as LucideCpu,
  Database as LucideDatabase,
  Download as LucideDownload,
  Ellipsis as LucideEllipsis,
  Eraser as LucideEraser,
  ExternalLink as LucideExternalLink,
  Eye as LucideEye,
  EyeOff as LucideEyeOff,
  FileCheck2 as LucideFileCheck2,
  FileCode2 as LucideFileCode2,
  FileDiff as LucideFileDiff,
  FilePenLine as LucideFilePenLine,
  FilePlus2 as LucideFilePlus2,
  Files as LucideFiles,
  FileSearch as LucideFileSearch,
  FileText as LucideFileText,
  Flame as LucideFlame,
  Folder as LucideFolder,
  FolderCode as LucideFolderCode,
  FolderGit2 as LucideFolderGit2,
  FolderInput as LucideFolderInput,
  FolderOpen as LucideFolderOpen,
  FolderPlus as LucideFolderPlus,
  Folders as LucideFolders,
  FolderSearch as LucideFolderSearch,
  FolderTree as LucideFolderTree,
  FoldVertical as LucideFoldVertical,
  FormInput as LucideFormInput,
  Gamepad2 as LucideGamepad2,
  Gauge as LucideGauge,
  GitBranch as LucideGitBranch,
  GitBranchPlus as LucideGitBranchPlus,
  GitCommitHorizontal as LucideGitCommitHorizontal,
  GitCompare as LucideGitCompare,
  GitCompareArrows as LucideGitCompareArrows,
  GitMerge as LucideGitMerge,
  GitPullRequest as LucideGitPullRequest,
  Globe as LucideGlobe,
  Globe2 as LucideGlobe2,
  GripVertical as LucideGripVertical,
  Hand as LucideHand,
  HardDrive as LucideHardDrive,
  Hash as LucideHash,
  Heading1 as LucideHeading1,
  Heading2 as LucideHeading2,
  Heading3 as LucideHeading3,
  History as LucideHistory,
  Hourglass as LucideHourglass,
  Image as LucideImage,
  ImagePlus as LucideImagePlus,
  Images as LucideImages,
  Info as LucideInfo,
  Italic as LucideItalic,
  Keyboard as LucideKeyboard,
  KeyRound as LucideKeyRound,
  Languages as LucideLanguages,
  Layers as LucideLayers,
  Layers2 as LucideLayers2,
  LayoutPanelLeft as LucideLayoutPanelLeft,
  LayoutTemplate as LucideLayoutTemplate,
  Library as LucideLibrary,
  Link as LucideLink,
  Link2 as LucideLink2,
  List as LucideList,
  ListChecks as LucideListChecks,
  ListOrdered as LucideListOrdered,
  ListTodo as LucideListTodo,
  ListTree as LucideListTree,
  Loader2 as LucideLoader2,
  LoaderCircle as LucideLoaderCircle,
  LogOut as LucideLogOut,
  Maximize2 as LucideMaximize2,
  MessageCirclePlus as LucideMessageCirclePlus,
  MessageSquare as LucideMessageSquare,
  MessageSquarePlus as LucideMessageSquarePlus,
  MessageSquareText as LucideMessageSquareText,
  MessagesSquare as LucideMessagesSquare,
  Minimize2 as LucideMinimize2,
  Minus as LucideMinus,
  MonitorCog as LucideMonitorCog,
  MoreHorizontal as LucideMoreHorizontal,
  Network as LucideNetwork,
  NotepadText as LucideNotepadText,
  PackageSearch as LucidePackageSearch,
  Palette as LucidePalette,
  PanelLeft as LucidePanelLeft,
  PanelLeftClose as LucidePanelLeftClose,
  PanelLeftOpen as LucidePanelLeftOpen,
  PanelRight as LucidePanelRight,
  PanelRightClose as LucidePanelRightClose,
  PanelRightOpen as LucidePanelRightOpen,
  Paperclip as LucidePaperclip,
  Pause as LucidePause,
  Pencil as LucidePencil,
  Pilcrow as LucidePilcrow,
  Pin as LucidePin,
  PinOff as LucidePinOff,
  Play as LucidePlay,
  Plug as LucidePlug,
  PlugZap as LucidePlugZap,
  Plus as LucidePlus,
  QrCode as LucideQrCode,
  Quote as LucideQuote,
  Radar as LucideRadar,
  RadioTower as LucideRadioTower,
  RefreshCw as LucideRefreshCw,
  RotateCcw as LucideRotateCcw,
  Route as LucideRoute,
  Rows3 as LucideRows3,
  Save as LucideSave,
  ScanEye as LucideScanEye,
  ScanText as LucideScanText,
  Scissors as LucideScissors,
  Search as LucideSearch,
  Server as LucideServer,
  Settings as LucideSettings,
  Settings2 as LucideSettings2,
  ShieldAlert as LucideShieldAlert,
  ShieldCheck as LucideShieldCheck,
  ShieldQuestion as LucideShieldQuestion,
  Sigma as LucideSigma,
  SkipForward as LucideSkipForward,
  SlidersHorizontal as LucideSlidersHorizontal,
  Sparkles as LucideSparkles,
  Square as LucideSquare,
  SquareCheckBig as LucideSquareCheckBig,
  SquarePen as LucideSquarePen,
  SquareTerminal as LucideSquareTerminal,
  SquareX as LucideSquareX,
  Strikethrough as LucideStrikethrough,
  Table as LucideTable,
  Table2 as LucideTable2,
  Tag as LucideTag,
  Target as LucideTarget,
  Terminal as LucideTerminal,
  TerminalSquare as LucideTerminalSquare,
  Timer as LucideTimer,
  Trash2 as LucideTrash2,
  TriangleAlert as LucideTriangleAlert,
  Undo2 as LucideUndo2,
  UnfoldVertical as LucideUnfoldVertical,
  Upload as LucideUpload,
  WandSparkles as LucideWandSparkles,
  Webhook as LucideWebhook,
  WrapText as LucideWrapText,
  Wrench as LucideWrench,
  X as LucideX,
  XCircle as LucideXCircle,
} from "lucide-react";
import { withIconDefaults } from "./with-icon-defaults";

export const Activity = withIconDefaults(LucideActivity);
export const AlertCircle = withIconDefaults(LucideAlertCircle);
export const AlignCenter = withIconDefaults(LucideAlignCenter);
export const AlignLeft = withIconDefaults(LucideAlignLeft);
export const AlignRight = withIconDefaults(LucideAlignRight);
export const Archive = withIconDefaults(LucideArchive);
export const ArchiveRestore = withIconDefaults(LucideArchiveRestore);
export const ArrowDown = withIconDefaults(LucideArrowDown);
export const ArrowDownToLine = withIconDefaults(LucideArrowDownToLine);
export const ArrowLeft = withIconDefaults(LucideArrowLeft);
export const ArrowLeftRight = withIconDefaults(LucideArrowLeftRight);
export const ArrowLeftToLine = withIconDefaults(LucideArrowLeftToLine);
export const ArrowRight = withIconDefaults(LucideArrowRight);
export const ArrowRightToLine = withIconDefaults(LucideArrowRightToLine);
export const ArrowUp = withIconDefaults(LucideArrowUp);
export const ArrowUpFromLine = withIconDefaults(LucideArrowUpFromLine);
export const ArrowUpToLine = withIconDefaults(LucideArrowUpToLine);
export const Ban = withIconDefaults(LucideBan);
export const BarChart3 = withIconDefaults(LucideBarChart3);
export const Blocks = withIconDefaults(LucideBlocks);
export const Bold = withIconDefaults(LucideBold);
export const BookMarked = withIconDefaults(LucideBookMarked);
export const BookOpen = withIconDefaults(LucideBookOpen);
export const Bot = withIconDefaults(LucideBot);
export const Box = withIconDefaults(LucideBox);
export const Braces = withIconDefaults(LucideBraces);
export const Brain = withIconDefaults(LucideBrain);
export const BrainCircuit = withIconDefaults(LucideBrainCircuit);
export const Cable = withIconDefaults(LucideCable);
export const Calculator = withIconDefaults(LucideCalculator);
export const CalendarClock = withIconDefaults(LucideCalendarClock);
export const CalendarPlus = withIconDefaults(LucideCalendarPlus);
export const Check = withIconDefaults(LucideCheck);
export const CheckCheck = withIconDefaults(LucideCheckCheck);
export const CheckCircle2 = withIconDefaults(LucideCheckCircle2);
export const CheckSquare = withIconDefaults(LucideCheckSquare);
export const CheckSquare2 = withIconDefaults(LucideCheckSquare2);
export const Cherry = withIconDefaults(LucideCherry);
export const ChevronDown = withIconDefaults(LucideChevronDown);
export const ChevronLeft = withIconDefaults(LucideChevronLeft);
export const ChevronRight = withIconDefaults(LucideChevronRight);
export const ChevronsDownUp = withIconDefaults(LucideChevronsDownUp);
export const ChevronsLeft = withIconDefaults(LucideChevronsLeft);
export const ChevronsLeftRight = withIconDefaults(LucideChevronsLeftRight);
export const ChevronsRight = withIconDefaults(LucideChevronsRight);
export const ChevronsUpDown = withIconDefaults(LucideChevronsUpDown);
export const ChevronUp = withIconDefaults(LucideChevronUp);
export const Circle = withIconDefaults(LucideCircle);
export const CircleAlert = withIconDefaults(LucideCircleAlert);
export const CircleCheck = withIconDefaults(LucideCircleCheck);
export const CircleDashed = withIconDefaults(LucideCircleDashed);
export const CircleDot = withIconDefaults(LucideCircleDot);
export const CircleGauge = withIconDefaults(LucideCircleGauge);
export const CircleStop = withIconDefaults(LucideCircleStop);
export const CircleX = withIconDefaults(LucideCircleX);
export const Clipboard = withIconDefaults(LucideClipboard);
export const ClipboardCopy = withIconDefaults(LucideClipboardCopy);
export const ClipboardList = withIconDefaults(LucideClipboardList);
export const ClipboardPaste = withIconDefaults(LucideClipboardPaste);
export const Clock = withIconDefaults(LucideClock);
export const Clock3 = withIconDefaults(LucideClock3);
export const CloudDownload = withIconDefaults(LucideCloudDownload);
export const CloudUpload = withIconDefaults(LucideCloudUpload);
export const Code = withIconDefaults(LucideCode);
export const Code2 = withIconDefaults(LucideCode2);
export const Columns2 = withIconDefaults(LucideColumns2);
export const Columns3 = withIconDefaults(LucideColumns3);
export const Compass = withIconDefaults(LucideCompass);
export const Copy = withIconDefaults(LucideCopy);
export const Cpu = withIconDefaults(LucideCpu);
export const Database = withIconDefaults(LucideDatabase);
export const Download = withIconDefaults(LucideDownload);
export const Ellipsis = withIconDefaults(LucideEllipsis);
export const Eraser = withIconDefaults(LucideEraser);
export const ExternalLink = withIconDefaults(LucideExternalLink);
export const Eye = withIconDefaults(LucideEye);
export const EyeOff = withIconDefaults(LucideEyeOff);
export const FileCheck2 = withIconDefaults(LucideFileCheck2);
export const FileCode2 = withIconDefaults(LucideFileCode2);
export const FileDiff = withIconDefaults(LucideFileDiff);
export const FilePenLine = withIconDefaults(LucideFilePenLine);
export const FilePlus2 = withIconDefaults(LucideFilePlus2);
export const Files = withIconDefaults(LucideFiles);
export const FileSearch = withIconDefaults(LucideFileSearch);
export const FileText = withIconDefaults(LucideFileText);
export const Flame = withIconDefaults(LucideFlame);
export const Folder = withIconDefaults(LucideFolder);
export const FolderCode = withIconDefaults(LucideFolderCode);
export const FolderGit2 = withIconDefaults(LucideFolderGit2);
export const FolderInput = withIconDefaults(LucideFolderInput);
export const FolderOpen = withIconDefaults(LucideFolderOpen);
export const FolderPlus = withIconDefaults(LucideFolderPlus);
export const Folders = withIconDefaults(LucideFolders);
export const FolderSearch = withIconDefaults(LucideFolderSearch);
export const FolderTree = withIconDefaults(LucideFolderTree);
export const FoldVertical = withIconDefaults(LucideFoldVertical);
export const FormInput = withIconDefaults(LucideFormInput);
export const Gamepad2 = withIconDefaults(LucideGamepad2);
export const Gauge = withIconDefaults(LucideGauge);
export const GitBranch = withIconDefaults(LucideGitBranch);
export const GitBranchPlus = withIconDefaults(LucideGitBranchPlus);
export const GitCommitHorizontal = withIconDefaults(LucideGitCommitHorizontal);
export const GitCompare = withIconDefaults(LucideGitCompare);
export const GitCompareArrows = withIconDefaults(LucideGitCompareArrows);
export const GitMerge = withIconDefaults(LucideGitMerge);
export const GitPullRequest = withIconDefaults(LucideGitPullRequest);
export const Globe = withIconDefaults(LucideGlobe);
export const Globe2 = withIconDefaults(LucideGlobe2);
export const GripVertical = withIconDefaults(LucideGripVertical);
export const Hand = withIconDefaults(LucideHand);
export const HardDrive = withIconDefaults(LucideHardDrive);
export const Hash = withIconDefaults(LucideHash);
export const Heading1 = withIconDefaults(LucideHeading1);
export const Heading2 = withIconDefaults(LucideHeading2);
export const Heading3 = withIconDefaults(LucideHeading3);
export const History = withIconDefaults(LucideHistory);
export const Hourglass = withIconDefaults(LucideHourglass);
export const Image = withIconDefaults(LucideImage);
export const ImagePlus = withIconDefaults(LucideImagePlus);
export const Images = withIconDefaults(LucideImages);
export const Info = withIconDefaults(LucideInfo);
export const Italic = withIconDefaults(LucideItalic);
export const Keyboard = withIconDefaults(LucideKeyboard);
export const KeyRound = withIconDefaults(LucideKeyRound);
export const Languages = withIconDefaults(LucideLanguages);
export const Layers = withIconDefaults(LucideLayers);
export const Layers2 = withIconDefaults(LucideLayers2);
export const LayoutPanelLeft = withIconDefaults(LucideLayoutPanelLeft);
export const LayoutTemplate = withIconDefaults(LucideLayoutTemplate);
export const Library = withIconDefaults(LucideLibrary);
export const Link = withIconDefaults(LucideLink);
export const Link2 = withIconDefaults(LucideLink2);
export const List = withIconDefaults(LucideList);
export const ListChecks = withIconDefaults(LucideListChecks);
export const ListOrdered = withIconDefaults(LucideListOrdered);
export const ListTodo = withIconDefaults(LucideListTodo);
export const ListTree = withIconDefaults(LucideListTree);
export const Loader2 = withIconDefaults(LucideLoader2);
export const LoaderCircle = withIconDefaults(LucideLoaderCircle);
export const LogOut = withIconDefaults(LucideLogOut);
export const Maximize2 = withIconDefaults(LucideMaximize2);
export const MessageCirclePlus = withIconDefaults(LucideMessageCirclePlus);
export const MessageSquare = withIconDefaults(LucideMessageSquare);
export const MessageSquarePlus = withIconDefaults(LucideMessageSquarePlus);
export const MessageSquareText = withIconDefaults(LucideMessageSquareText);
export const MessagesSquare = withIconDefaults(LucideMessagesSquare);
export const Minimize2 = withIconDefaults(LucideMinimize2);
export const Minus = withIconDefaults(LucideMinus);
export const MonitorCog = withIconDefaults(LucideMonitorCog);
export const MoreHorizontal = withIconDefaults(LucideMoreHorizontal);
export const Network = withIconDefaults(LucideNetwork);
export const NotepadText = withIconDefaults(LucideNotepadText);
export const PackageSearch = withIconDefaults(LucidePackageSearch);
export const Palette = withIconDefaults(LucidePalette);
export const PanelLeft = withIconDefaults(LucidePanelLeft);
export const PanelLeftClose = withIconDefaults(LucidePanelLeftClose);
export const PanelLeftOpen = withIconDefaults(LucidePanelLeftOpen);
export const PanelRight = withIconDefaults(LucidePanelRight);
export const PanelRightClose = withIconDefaults(LucidePanelRightClose);
export const PanelRightOpen = withIconDefaults(LucidePanelRightOpen);
export const Paperclip = withIconDefaults(LucidePaperclip);
export const Pause = withIconDefaults(LucidePause);
export const Pencil = withIconDefaults(LucidePencil);
export const Pilcrow = withIconDefaults(LucidePilcrow);
export const Pin = withIconDefaults(LucidePin);
export const PinOff = withIconDefaults(LucidePinOff);
export const Play = withIconDefaults(LucidePlay);
export const Plug = withIconDefaults(LucidePlug);
export const PlugZap = withIconDefaults(LucidePlugZap);
export const Plus = withIconDefaults(LucidePlus);
export const QrCode = withIconDefaults(LucideQrCode);
export const Quote = withIconDefaults(LucideQuote);
export const Radar = withIconDefaults(LucideRadar);
export const RadioTower = withIconDefaults(LucideRadioTower);
export const RefreshCw = withIconDefaults(LucideRefreshCw);
export const RotateCcw = withIconDefaults(LucideRotateCcw);
export const Route = withIconDefaults(LucideRoute);
export const Rows3 = withIconDefaults(LucideRows3);
export const Save = withIconDefaults(LucideSave);
export const ScanEye = withIconDefaults(LucideScanEye);
export const ScanText = withIconDefaults(LucideScanText);
export const Scissors = withIconDefaults(LucideScissors);
export const Search = withIconDefaults(LucideSearch);
export const Server = withIconDefaults(LucideServer);
export const Settings = withIconDefaults(LucideSettings);
export const Settings2 = withIconDefaults(LucideSettings2);
export const ShieldAlert = withIconDefaults(LucideShieldAlert);
export const ShieldCheck = withIconDefaults(LucideShieldCheck);
export const ShieldQuestion = withIconDefaults(LucideShieldQuestion);
export const Sigma = withIconDefaults(LucideSigma);
export const SkipForward = withIconDefaults(LucideSkipForward);
export const SlidersHorizontal = withIconDefaults(LucideSlidersHorizontal);
export const Sparkles = withIconDefaults(LucideSparkles);
export const Square = withIconDefaults(LucideSquare);
export const SquareCheckBig = withIconDefaults(LucideSquareCheckBig);
export const SquarePen = withIconDefaults(LucideSquarePen);
export const SquareTerminal = withIconDefaults(LucideSquareTerminal);
export const SquareX = withIconDefaults(LucideSquareX);
export const Strikethrough = withIconDefaults(LucideStrikethrough);
export const Table = withIconDefaults(LucideTable);
export const Table2 = withIconDefaults(LucideTable2);
export const Tag = withIconDefaults(LucideTag);
export const Target = withIconDefaults(LucideTarget);
export const Terminal = withIconDefaults(LucideTerminal);
export const TerminalSquare = withIconDefaults(LucideTerminalSquare);
export const Timer = withIconDefaults(LucideTimer);
export const Trash2 = withIconDefaults(LucideTrash2);
export const TriangleAlert = withIconDefaults(LucideTriangleAlert);
export const Undo2 = withIconDefaults(LucideUndo2);
export const UnfoldVertical = withIconDefaults(LucideUnfoldVertical);
export const Upload = withIconDefaults(LucideUpload);
export const WandSparkles = withIconDefaults(LucideWandSparkles);
export const Webhook = withIconDefaults(LucideWebhook);
export const WrapText = withIconDefaults(LucideWrapText);
export const Wrench = withIconDefaults(LucideWrench);
export const X = withIconDefaults(LucideX);
export const XCircle = withIconDefaults(LucideXCircle);
