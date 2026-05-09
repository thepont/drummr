import { render, screen } from '@testing-library/react'
import { describe, it, expect, vi } from 'vitest'
import { ModulationPanel } from './ModulationPanel'

describe('ModulationPanel', () => {
  it('renders LFO controls', () => {
    render(
      <ModulationPanel 
        lfo1_freq={1.0} 
        lfo2_freq={5.0} 
        onChangeLfo={() => {}} 
      />
    )
    expect(screen.getByText(/LFO 1 Rate/)).toBeInTheDocument()
    expect(screen.getByText(/LFO 2 Rate/)).toBeInTheDocument()
    expect(screen.getByText(/1.00 Hz/)).toBeInTheDocument()
    expect(screen.getByText(/5.00 Hz/)).toBeInTheDocument()
  })
})
