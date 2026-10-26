import { forwardRef, type ButtonHTMLAttributes } from "react";

const VARIANT_CLASS = {
  primary: "asb-btn-primary",
  secondary: "asb-btn-secondary",
  ghost: "asb-btn-ghost",
  danger: "asb-btn-danger",
  icon: "asb-btn-icon",
  dangerIcon: "asb-btn-icon asb-btn-icon-danger",
  plus: "asb-btn-plus",
  back: "asb-btn-back",
  unstyled: "asb-btn-unstyled",
} as const;

export type ButtonVariant = keyof typeof VARIANT_CLASS;

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant: ButtonVariant;
  size?: "standard" | "compact";
}

/**
 * Global action control. Owns the geometry contract: every instance emits
 * .asb-btn plus its variant classes and an optional compact size class;
 * styles/base/buttons.css owns their geometry and surfaces.
 */
export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  function Button({ variant, size = "standard", className, type = "button", ...props }, ref) {
    return (
      <button
        ref={ref}
        type={type}
        className={["asb-btn", VARIANT_CLASS[variant], size === "compact" && "asb-btn-compact", className]
          .filter(Boolean)
          .join(" ")}
        {...props}
      />
    );
  },
);
