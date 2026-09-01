import { forwardRef, type TextareaHTMLAttributes } from "react";

interface Props extends TextareaHTMLAttributes<HTMLTextAreaElement> {
  /** 等宽代码变体：逐行核对的内容（如可选模型列表）。 */
  code?: boolean;
}

/**
 * Multi-line sibling of Input, ported from the spiralcoder textarea: native
 * attributes pass through; the field look is shared with .asb-input and the
 * multi-line sizing with .asb-textarea in styles/base.css.
 */
export const Textarea = forwardRef<HTMLTextAreaElement, Props>(function Textarea(
  { code = false, ...props },
  ref,
) {
  return (
    <textarea
      ref={ref}
      className={code ? "asb-input asb-code asb-textarea" : "asb-input asb-textarea"}
      {...props}
    />
  );
});
