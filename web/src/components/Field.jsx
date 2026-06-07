export function Field({ label, hint, children }) {
  return (
    <label className="flex flex-col gap-1.5">
      <span className="text-xs font-semibold uppercase tracking-[0.16em] text-ink-faint">{label}</span>
      {children}
      {hint && <span className="text-xs text-ink-faint">{hint}</span>}
    </label>
  );
}

const inputClasses =
  'w-full rounded-xl border border-line bg-card px-3.5 py-2.5 text-sm text-ink placeholder:text-ink-faint outline-none transition focus:border-coral focus:ring-2 focus:ring-coral/15';

export function TextInput(props) {
  return <input {...props} className={`${inputClasses} ${props.className || ''}`} />;
}

export function Select({ options, placeholder, ...props }) {
  return (
    <select {...props} className={`${inputClasses} ${props.className || ''}`}>
      {placeholder && <option value="">{placeholder}</option>}
      {options.map((opt) => (
        <option key={opt} value={opt}>
          {opt}
        </option>
      ))}
    </select>
  );
}

export function Toggle({ checked, onChange, label, sub, disabled }) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={`flex items-center gap-3 rounded-2xl border px-4 py-3 text-left transition ${
        checked ? 'border-coral/40 bg-coral-soft' : 'border-line bg-card'
      } ${disabled ? 'cursor-not-allowed opacity-50' : 'hover:border-ink/20'}`}
    >
      <span
        className={`relative inline-flex h-6 w-11 shrink-0 items-center rounded-full transition ${
          checked ? 'bg-coral' : 'bg-line'
        }`}
      >
        <span
          className={`inline-block h-4.5 w-4.5 transform rounded-full bg-white shadow transition ${
            checked ? 'translate-x-6' : 'translate-x-1'
          }`}
        />
      </span>
      <span>
        <span className="block text-sm font-medium text-ink">{label}</span>
        {sub && <span className="block text-xs text-ink-faint">{sub}</span>}
      </span>
    </button>
  );
}
