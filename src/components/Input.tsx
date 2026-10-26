import { forwardRef, type InputHTMLAttributes } from "react";

interface Props extends Omit<InputHTMLAttributes<HTMLInputElement>, "size" | "className"> {
  /** 等宽代码变体：模型 ID、环境变量名等逐字符核对的内容。 */
  code?: boolean;
}

/**
 * Text-field control: native attributes
 * pass straight through while the field look (hairline border, control radius,
 * canvas fill, placeholder, hover, disabled) is owned by
 * .asb-input in styles/base/field-controls.css; every value comes from styles/tokens.css.
 * 字段外观类由本组件独占；不接受 className。宽度与排布属于父级容器，标准高度来自共享控件样式。
 */
export const Input = forwardRef<HTMLInputElement, Props>(
  function Input({ code = false, ...props }, ref) {
    return <input ref={ref} className={code ? "asb-input asb-code" : "asb-input"} {...props} />;
  },
);
