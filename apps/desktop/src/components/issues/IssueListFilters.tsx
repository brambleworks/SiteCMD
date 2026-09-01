import { ChevronDown, Search } from "lucide-react";

export function FilterSelect({
  label,
  ariaLabel,
  value,
  options,
  onChange,
  disabled = false,
}: {
  label: string;
  ariaLabel: string;
  value: string;
  options: Array<{ value: string; label: string }>;
  onChange: (value: string) => void;
  disabled?: boolean;
}) {
  return (
    <label className="filter-select-field">
      <span className="eyebrow filter-select-label text-muted-foreground">{label}</span>
      <div className="filter-select-shell">
        <select
          aria-label={ariaLabel}
          value={value}
          onChange={(event) => onChange(event.target.value)}
          disabled={disabled}
          className="field-control field-control--select">
          {options.map((option) => (
            <option key={option.value} value={option.value}>
              {option.label}
            </option>
          ))}
        </select>
        <ChevronDown className="filter-select-chevron" />
      </div>
    </label>
  );
}

export function FilterSearch({
  label,
  ariaLabel,
  placeholder,
  value,
  onChange,
}: {
  label: string;
  ariaLabel: string;
  placeholder: string;
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <label className="filter-select-field">
      <span className="eyebrow filter-select-label text-muted-foreground">{label}</span>
      <div className="filter-select-shell">
        <Search className="filter-search-icon" aria-hidden="true" />
        <input
          type="search"
          aria-label={ariaLabel}
          placeholder={placeholder}
          value={value}
          onChange={(event) => onChange(event.target.value)}
          className="field-control field-control--with-icon"
        />
      </div>
    </label>
  );
}
