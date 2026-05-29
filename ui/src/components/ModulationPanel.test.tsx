import { render, screen } from '@testing-library/react'
import { describe, it, expect } from 'vitest'
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
    expect(screen.getAllByText(/LFO 1/)[0]).toBeInTheDocument()
    expect(screen.getAllByText(/LFO 2/)[0]).toBeInTheDocument()
    expect(screen.getByText(/1.00 Hz/)).toBeInTheDocument()
    expect(screen.getByText(/5.00 Hz/)).toBeInTheDocument()
  })
})
