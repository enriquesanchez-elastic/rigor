import React from 'react';
import { render, fireEvent } from '@testing-library/react';
import { Button } from './Button';

it('calls onClick when clicked', () => {
  const handleClick = jest.fn();
  const { container } = render(<Button label="Click me" onClick={handleClick} />);

  const button = container.querySelector('[data-testid="button"]');
  if (button) fireEvent.click(button);

  expect(handleClick).toHaveBeenCalledTimes(1);
});

it('renders with getByTestId', () => {
  const { getByTestId } = render(<Button label="Submit" />);
  const btn = getByTestId('button');
  expect(btn).toBeInTheDocument();
  expect(btn).toHaveTextContent('Submit');
});

it('disabled button does not call onClick', () => {
  const handleClick = jest.fn();
  const { getByTestId } = render(<Button label="Click" onClick={handleClick} disabled />);
  fireEvent.click(getByTestId('button'));
  expect(handleClick).not.toHaveBeenCalled();
});
