import type { InputHTMLAttributes } from "react";

interface Props extends Omit<InputHTMLAttributes<HTMLInputElement>, "size"> {
  /** 等宽代码变体：模型 ID、环境变量名等逐字符核对的内容。 */
  code?: boolean;
}

/**
 * Text-field control ported from the spiralcoder reference: native attributes
 * pass straight through while the field look (hairline border, control radius,
 * canvas fill, placeholder, hover, focus ring, disabled) is owned by
 * .asb-input in styles/base.css; every value comes from styles/tokens.css.
 */
export function Input({ code = false, ...props }: Props) {
  return <input className={code ? "asb-input asb-code" : "asb-input"} {...props} />;
}
