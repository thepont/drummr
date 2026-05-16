import { render, screen } from '@testing-library/react'
import { describe, it, expect } from 'vitest'
import { EnvelopeEditor } from './EnvelopeEditor'

describe('EnvelopeEditor', () => {
  it('renders labels for attack and decay', () => {
    render(
      <EnvelopeEditor 
        attack={100} 
        decay={400} 
        onChange={() => {}} 
      />
    )
    expect(screen.getByText(/A: 100ms/)).toBeInTheDocument()
    expect(screen.getByText(/D: 400ms/)).toBeInTheDocument()
  })

  it('renders an SVG with a handle', () => {
    const { container } = render(
      <EnvelopeEditor 
        attack={100} 
        decay={400} 
        onChange={() => {}} 
      />
    )
    expect(container.querySelector('svg')).toBeInTheDocument()
    expect(container.querySelector('circle')).toBeInTheDocument()
  })
})
