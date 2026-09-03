/** One segment of the shared `asb-segments` radio group. */
export function RadioOption({
  name,
  checked,
  disabled,
  label,
  onChange,
}: {
  name: string;
  checked: boolean;
  disabled: boolean;
  label: string;
  onChange: () => void;
}) {
  return (
    <label className={`asb-seg-opt${checked ? " is-active" : ""}`}>
      <input
        type="radio"
        name={name}
        checked={checked}
        disabled={disabled}
        onChange={onChange}
      />
      {label}
    </label>
  );
}
