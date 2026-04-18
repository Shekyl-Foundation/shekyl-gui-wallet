import { useState } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";

interface CollapsibleSectionProps {
  icon?: React.ComponentType<{ className?: string }>;
  title: string;
  children: React.ReactNode;
  /**
   * Controlled mode: when provided, the parent owns open/closed state.
   * `onToggle` must be provided alongside.
   */
  open?: boolean;
  onToggle?: () => void;
  /**
   * Uncontrolled mode: initial open state. Ignored when `open` is set.
   */
  defaultOpen?: boolean;
  /**
   * Optional override for the outer container className so callers
   * (e.g. Help.tsx) can retain the existing `card` styling while new
   * call sites like CreateWallet / ImportWallet use inline styling.
   */
  className?: string;
}

/**
 * Collapsible section header + panel. Extracted from `pages/Help.tsx`
 * so the same affordance can power the Advanced directory picker on
 * the wallet create/import flows without duplicating chevron markup.
 *
 * Supports both controlled (`open` + `onToggle`, used by the Help page
 * where only one section is open at a time) and uncontrolled
 * (`defaultOpen`, used by isolated "Advanced" disclosures).
 */
export default function CollapsibleSection({
  icon: Icon,
  title,
  children,
  open: controlledOpen,
  onToggle,
  defaultOpen = false,
  className = "card",
}: CollapsibleSectionProps) {
  const [uncontrolledOpen, setUncontrolledOpen] = useState(defaultOpen);
  const isControlled = controlledOpen !== undefined;
  const isOpen = isControlled ? controlledOpen : uncontrolledOpen;

  function handleToggle() {
    if (isControlled) {
      onToggle?.();
    } else {
      setUncontrolledOpen((prev) => !prev);
    }
  }

  return (
    <div className={className}>
      <button
        type="button"
        onClick={handleToggle}
        className="flex w-full items-center gap-3 text-left"
      >
        {Icon && <Icon className="h-5 w-5 shrink-0 text-gold-400" />}
        <span className="flex-1 text-sm font-semibold text-white">{title}</span>
        {isOpen ? (
          <ChevronDown className="h-4 w-4 text-purple-400" />
        ) : (
          <ChevronRight className="h-4 w-4 text-purple-400" />
        )}
      </button>
      {isOpen && (
        <div className="mt-4 space-y-3 border-t border-purple-700/50 pt-4 text-xs leading-relaxed text-purple-200">
          {children}
        </div>
      )}
    </div>
  );
}
