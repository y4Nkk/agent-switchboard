import { useContext, type ReactNode } from "react";
import { Ellipsis } from "lucide-react";
import { Menu, MenuItem, MenuTrigger, Popover, Pressable } from "react-aria-components";
import { Button } from "./Button";
import { OverlayMotionContext } from "./overlay-motion";
import { Tooltip } from "./Tooltip";

export interface ActionMenuItem {
  id: string;
  label: string;
  onAction: () => void;
  disabled?: boolean;
  danger?: boolean;
}

interface Props {
  /** Omit the visible label for the icon-only trigger. */
  label?: ReactNode;
  ariaLabel: string;
  items: ActionMenuItem[];
  disabled?: boolean;
}

export function ActionMenu({ label, ariaLabel, items, disabled }: Props) {
  const iconOnly = label == null;
  const animate = useContext(OverlayMotionContext);
  const trigger = (
    <Pressable isDisabled={disabled}>
      <Button variant={iconOnly ? "icon" : "secondary"} aria-label={ariaLabel} disabled={disabled}>
        {label ?? <Ellipsis strokeWidth={1.7} aria-hidden="true" />}
      </Button>
    </Pressable>
  );
  return (
    <MenuTrigger>
      {iconOnly ? <Tooltip label={ariaLabel} side="bottom">{trigger}</Tooltip> : trigger}
      <Popover placement="bottom end" className="asb-action-menu" data-overlay-motion={animate || undefined}>
        <Menu aria-label={ariaLabel}>
          {items.map((item) => (
            <MenuItem key={item.id} id={item.id} isDisabled={disabled || item.disabled}
              className={item.danger ? "asb-action-menu-item is-danger" : "asb-action-menu-item"}
              onAction={item.onAction}>{item.label}</MenuItem>
          ))}
        </Menu>
      </Popover>
    </MenuTrigger>
  );
}
