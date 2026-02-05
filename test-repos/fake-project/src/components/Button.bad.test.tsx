import React from 'react';
import { render, fireEvent } from '@testing-library/react';
import { Button } from './Button';

describe('RTL anti-patterns', () => {
  it('uses container.querySelector instead of screen', () => {
    const { container } = render(<Button label="Click" />);
    const btn = container.querySelector('button');
    expect(btn).toHaveTextContent('Click');
  });

  it('uses getByTestId for everything', () => {
    const { getByTestId } = render(<Button label="Submit" />);
    const btn = getByTestId('button');
    expect(btn).toBeInTheDocument();
  });

  it('only fireEvent no userEvent', () => {
    const onClick = jest.fn();
    const { getByTestId } = render(<Button label="OK" onClick={onClick} />);
    fireEvent.click(getByTestId('button'));
    expect(onClick).toHaveBeenCalled();
  });

  it('querySelector by class', () => {
    const { container } = render(<Button label="X" variant="primary" />);
    const el = container.querySelector('.btn-primary');
    expect(el).toBeTruthy();
  });

  it('getByTestId as primary query', () => {
    const { getByTestId } = render(<Button label="Save" disabled />);
    const button = getByTestId('button');
    expect(button).toBeDisabled();
  });
});
