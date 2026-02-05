import React from 'react';

export type ButtonVariant = 'primary' | 'secondary' | 'danger';

export interface ButtonProps {
  label: string;
  onClick?: () => void;
  disabled?: boolean;
  variant?: ButtonVariant;
}

export function Button({ label, onClick, disabled = false, variant = 'primary' }: ButtonProps): React.ReactElement {
  return (
    <button
      type="button"
      role="button"
      data-testid="button"
      className={`btn btn-${variant}`}
      onClick={onClick}
      disabled={disabled}
    >
      {label}
    </button>
  );
}
