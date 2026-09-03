import { forwardRef, type ButtonHTMLAttributes } from "react";

const VARIANT_CLASS = {
  primary: "asb-btn-primary",
  secondary: "asb-btn-secondary",
  danger: "asb-btn-danger",
  icon: "asb-btn-icon",
  plus: "asb-btn-plus",
  back: "asb-btn-back",
} as const;

export type ButtonVariant = keyof typeof VARIANT_CLASS;

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant: ButtonVariant;
}

/**
 * Global action control. Owns the geometry contract: every instance emits
 * .asb-btn plus exactly one variant surface class owned by styles/base.css.
 */
export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  function Button({ variant, className, type = "button", ...props }, ref) {
    return (
      <button
        ref={ref}
        type={type}
        className={["asb-btn", VARIANT_CLASS[variant], className]
          .filter(Boolean)
          .join(" ")}
        {...props}
      />
    );
  },
);
