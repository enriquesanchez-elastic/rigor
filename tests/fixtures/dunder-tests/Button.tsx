// Button component with __tests__ folder structure
import React from 'react';

export interface ButtonProps {
  label: string;
  onClick: () => void;
  disabled?: boolean;
  variant?: 'primary' | 'secondary' | 'danger';
  size?: 'small' | 'medium' | 'large';
}

export function Button({ 
  label, 
  onClick, 
  disabled = false, 
  variant = 'primary',
  size = 'medium' 
}: ButtonProps) {
  const baseClass = 'btn';
  const variantClass = `btn-${variant}`;
  const sizeClass = `btn-${size}`;
  const disabledClass = disabled ? 'btn-disabled' : '';
  
  const className = [baseClass, variantClass, sizeClass, disabledClass]
    .filter(Boolean)
    .join(' ');

  const handleClick = () => {
    if (!disabled) {
      onClick();
    }
  };

  return (
    <button 
      className={className}
      onClick={handleClick}
      disabled={disabled}
      aria-disabled={disabled}
    >
      {label}
    </button>
  );
}
