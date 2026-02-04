// Test file using __tests__ folder structure
import React from 'react';
import { render, screen, fireEvent } from '@testing-library/react';
import { Button } from '../Button';

describe('Button', () => {
  it('renders with correct label', () => {
    render(<Button label="Click me" onClick={() => {}} />);
    
    expect(screen.getByRole('button')).toHaveTextContent('Click me');
  });

  it('calls onClick when clicked', () => {
    const handleClick = jest.fn();
    render(<Button label="Click me" onClick={handleClick} />);
    
    fireEvent.click(screen.getByRole('button'));
    
    expect(handleClick).toHaveBeenCalledTimes(1);
  });

  it('does not call onClick when disabled', () => {
    const handleClick = jest.fn();
    render(<Button label="Click me" onClick={handleClick} disabled />);
    
    fireEvent.click(screen.getByRole('button'));
    
    expect(handleClick).not.toHaveBeenCalled();
  });

  it('applies correct variant class', () => {
    render(<Button label="Danger" onClick={() => {}} variant="danger" />);
    
    const button = screen.getByRole('button');
    expect(button).toHaveClass('btn-danger');
  });

  it('applies correct size class', () => {
    render(<Button label="Small" onClick={() => {}} size="small" />);
    
    const button = screen.getByRole('button');
    expect(button).toHaveClass('btn-small');
  });

  it('has aria-disabled attribute when disabled', () => {
    render(<Button label="Disabled" onClick={() => {}} disabled />);
    
    const button = screen.getByRole('button');
    expect(button).toHaveAttribute('aria-disabled', 'true');
  });

  it('applies disabled class when disabled', () => {
    render(<Button label="Disabled" onClick={() => {}} disabled />);
    
    const button = screen.getByRole('button');
    expect(button).toHaveClass('btn-disabled');
  });

  it('uses primary variant by default', () => {
    render(<Button label="Default" onClick={() => {}} />);
    
    const button = screen.getByRole('button');
    expect(button).toHaveClass('btn-primary');
  });

  it('uses medium size by default', () => {
    render(<Button label="Default" onClick={() => {}} />);
    
    const button = screen.getByRole('button');
    expect(button).toHaveClass('btn-medium');
  });
});
