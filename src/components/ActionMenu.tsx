import type { ReactNode } from "react";
import { Menu, MenuItem, MenuTrigger, Popover, Pressable } from "react-aria-components";
import { Button } from "./Button";

export interface ActionMenuItem {
  id: string;
  label: string;
  onAction: () => void;
  disabled?: boolean;
  danger?: boolean;
}

interface Props {
  label: ReactNode;
  ariaLabel: string;
  items: ActionMenuItem[];
  disabled?: boolean;
}

export function ActionMenu({ label, ariaLabel, items, disabled }: Props) {
  return (
    <MenuTrigger>
      <Pressable isDisabled={disabled}>
        <Button variant="secondary" aria-label={ariaLabel} disabled={disabled}>{label}</Button>
      </Pressable>
      <Popover placement="bottom end" className="asb-action-menu">
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
