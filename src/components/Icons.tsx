/** Hand-rolled 16px stroke icons — no icon package, nothing fetched at runtime. */
type Props = { className?: string; size?: number };

const base = (size: number) => ({
  width: size,
  height: size,
  viewBox: "0 0 16 16",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.4,
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
});

export const SearchIcon = ({ className, size = 14 }: Props) => (
  <svg {...base(size)} className={className}>
    <circle cx="7" cy="7" r="4.25" />
    <path d="M10.2 10.2 13.5 13.5" />
  </svg>
);

export const GearIcon = ({ className, size = 15 }: Props) => (
  <svg {...base(size)} className={className}>
    <circle cx="8" cy="8" r="2.1" />
    <path d="M8 1.6v1.7M8 12.7v1.7M14.4 8h-1.7M3.3 8H1.6M12.5 3.5l-1.2 1.2M4.7 11.3l-1.2 1.2M12.5 12.5l-1.2-1.2M4.7 4.7 3.5 3.5" />
  </svg>
);

export const ChevronDown = ({ className, size = 12 }: Props) => (
  <svg {...base(size)} className={className}>
    <path d="M4 6.25 8 10l4-3.75" />
  </svg>
);

export const ChevronRight = ({ className, size = 12 }: Props) => (
  <svg {...base(size)} className={className}>
    <path d="M6.25 4 10 8l-3.75 4" />
  </svg>
);

export const TrashIcon = ({ className, size = 14 }: Props) => (
  <svg {...base(size)} className={className}>
    <path d="M2.8 4.2h10.4M6.4 4.2V2.9h3.2v1.3M4.2 4.2l.6 8.2c0 .4.4.7.8.7h4.8c.4 0 .8-.3.8-.7l.6-8.2" />
  </svg>
);

export const FolderIcon = ({ className, size = 14 }: Props) => (
  <svg {...base(size)} className={className}>
    <path d="M1.9 12.4V3.6c0-.4.3-.7.7-.7h3.1l1.4 1.6h6c.4 0 .7.3.7.7v7.2c0 .4-.3.7-.7.7H2.6a.7.7 0 0 1-.7-.7Z" />
  </svg>
);

export const TerminalIcon = ({ className, size = 14 }: Props) => (
  <svg {...base(size)} className={className}>
    <rect x="1.8" y="2.8" width="12.4" height="10.4" rx="1.6" />
    <path d="M4.6 6.2 6.6 8l-2 1.8M8.6 10.2h3" />
  </svg>
);

export const ForkIcon = ({ className, size = 14 }: Props) => (
  <svg {...base(size)} className={className}>
    <circle cx="4.4" cy="3.6" r="1.6" />
    <circle cx="11.6" cy="3.6" r="1.6" />
    <circle cx="4.4" cy="12.4" r="1.6" />
    <path d="M4.4 5.2v5.6M11.6 5.2v1.4c0 1.3-1 2.4-2.4 2.4H6.8" />
  </svg>
);

export const WarnIcon = ({ className, size = 14 }: Props) => (
  <svg {...base(size)} className={className}>
    <path d="M8 2.6 14.2 13H1.8L8 2.6Z" />
    <path d="M8 6.6v3M8 11.4h.01" />
  </svg>
);

export const UndoIcon = ({ className, size = 14 }: Props) => (
  <svg {...base(size)} className={className}>
    <path d="M3 6.5h6.2a3.6 3.6 0 0 1 0 7.2H6" />
    <path d="M5.4 4.1 3 6.5l2.4 2.4" />
  </svg>
);

export const CheckIcon = ({ className, size = 14 }: Props) => (
  <svg {...base(size)} className={className}>
    <path d="M3.2 8.4 6.4 11.6l6.4-7.2" />
  </svg>
);

export const CloseIcon = ({ className, size = 12 }: Props) => (
  <svg {...base(size)} className={className}>
    <path d="M3.5 3.5 12.5 12.5M12.5 3.5 3.5 12.5" />
  </svg>
);

export const MinimizeIcon = ({ className, size = 12 }: Props) => (
  <svg {...base(size)} className={className}>
    <path d="M3.5 8h9" />
  </svg>
);

export const MaximizeIcon = ({ className, size = 12 }: Props) => (
  <svg {...base(size)} className={className}>
    <rect x="3.6" y="3.6" width="8.8" height="8.8" rx="1.4" />
  </svg>
);

export const RestoreIcon = ({ className, size = 12 }: Props) => (
  <svg {...base(size)} className={className}>
    <rect x="3.2" y="5.2" width="7.6" height="7.6" rx="1.3" />
    <path d="M5.8 5.2V4.5c0-.7.6-1.3 1.3-1.3h4.4c.7 0 1.3.6 1.3 1.3v4.4c0 .7-.6 1.3-1.3 1.3h-.7" />
  </svg>
);

/** The stacked-bars mark from the app icon, reused in the sidebar. */
export const DeckMark = ({ className, size = 16 }: Props) => (
  <svg width={size} height={size} viewBox="0 0 16 16" className={className}>
    <rect x="2" y="3.4" width="12" height="2.4" rx="1.2" fill="currentColor" />
    <rect
      x="2"
      y="6.8"
      width="9"
      height="2.4"
      rx="1.2"
      fill="currentColor"
      opacity="0.6"
    />
    <rect
      x="2"
      y="10.2"
      width="6"
      height="2.4"
      rx="1.2"
      fill="currentColor"
      opacity="0.32"
    />
  </svg>
);
